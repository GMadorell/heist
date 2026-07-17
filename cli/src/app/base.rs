use crate::domain::error::StateError;
use crate::domain::value::NonBlankValue;
use crate::ports::git::{GitRepository, PrState};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

pub enum BaseResolution {
    Null,
    Live {
        base_ref: NonBlankValue,
        // R5 (stale-base detection: local base ref outrunning its remote
        // counterpart) is intentionally not implemented yet; deferred to a
        // follow-up. This variant only reports liveness, not staleness.
        /// `Some` when the PR-state check couldn't run (missing `gh`, no
        /// auth, etc.) rather than having actually confirmed the base's PR
        /// state; the base ref still resolves, so it's usable.
        verification_error: Option<String>,
    },
    Expired {
        base_ref: NonBlankValue,
    },
    Abandoned {
        base_ref: NonBlankValue,
    },
}

pub enum ResolveError {
    NoState,
    Load(StateError),
    RefMissingWithOpenPr { base_ref: String },
    RefMissingNoPr { base_ref: String },
    Ambiguous { base_ref: String },
}

pub fn resolve(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &str,
) -> Result<BaseResolution, ResolveError> {
    if !state_repo.exists(slug) {
        return Err(ResolveError::NoState);
    }

    let state = state_repo.load(slug).map_err(ResolveError::Load)?;

    let Some(base_value) = state.base else {
        return Ok(BaseResolution::Null);
    };
    let base_ref = base_value.as_ref();
    let main_branch = git.default_branch(repo_root);

    // Check if ref exists
    let ref_exists = git.resolve_ref(repo_root, base_ref).is_ok();

    // Ancestry pre-check (only if ref exists): a fast-forward/non-squash
    // merge may already have landed, in which case there's no need for a
    // `gh` call at all.
    if ref_exists {
        let ancestry = git.is_ancestor(repo_root, base_ref, &format!("origin/{}", main_branch));
        if matches!(ancestry, Ok(true)) {
            return Ok(BaseResolution::Expired {
                base_ref: base_value,
            });
        }
    }

    // PR state check
    match git.pr_state(repo_root, base_ref) {
        Ok(PrState::Merged) => Ok(BaseResolution::Expired {
            base_ref: base_value,
        }),
        Ok(PrState::ClosedUnmerged) => Ok(BaseResolution::Abandoned {
            base_ref: base_value,
        }),
        Ok(PrState::Open) => {
            if ref_exists {
                Ok(BaseResolution::Live {
                    base_ref: base_value,
                    verification_error: None,
                })
            } else {
                // The branch is gone but a PR still reports open: the human
                // must reconcile that, we can't guess the effective base.
                Err(ResolveError::RefMissingWithOpenPr {
                    base_ref: base_ref.to_string(),
                })
            }
        }
        Ok(PrState::None) => {
            if ref_exists {
                Ok(BaseResolution::Live {
                    base_ref: base_value,
                    verification_error: None,
                })
            } else {
                // No ref and no PR ever found: the base branch was most
                // likely deleted. Don't claim a PR is open (it isn't).
                Err(ResolveError::RefMissingNoPr {
                    base_ref: base_ref.to_string(),
                })
            }
        }
        Err(e) => {
            if ref_exists {
                Ok(BaseResolution::Live {
                    base_ref: base_value,
                    verification_error: Some(e.to_string()),
                })
            } else {
                Err(ResolveError::Ambiguous {
                    base_ref: base_ref.to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::DateValue;
    use crate::ports::git::GitError;

    fn test_state(slug: &str) -> State {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        State::new(slug, today).expect("valid slug")
    }

    #[test]
    fn resolve_skips_ancestry_check_when_ref_missing_and_asks_gh_directly() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .failing_resolve_ref_for(
                "heist/piece-01",
                GitError::MergeCheck {
                    message: "missing".into(),
                },
            )
            .with_pr_state("heist/piece-01", PrState::Merged);

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Ok(BaseResolution::Expired { .. })));
        assert_eq!(git.is_ancestor_call_count(), 0);
    }

    #[test]
    fn resolve_returns_null_when_state_base_is_none() {
        let state = test_state("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new();

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Ok(BaseResolution::Null)));
    }

    #[test]
    fn resolve_returns_expired_when_ref_resolves_and_is_ancestor_of_main() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_ancestor("heist/piece-01", "origin/main");

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Ok(BaseResolution::Expired { .. })));
    }

    #[test]
    fn resolve_returns_expired_when_gh_reports_merged() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::Merged);

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Ok(BaseResolution::Expired { .. })));
    }

    #[test]
    fn resolve_returns_abandoned_when_gh_reports_closed_unmerged() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Ok(BaseResolution::Abandoned { .. })));
    }

    #[test]
    fn resolve_returns_live_when_ref_resolves_and_gh_reports_open() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::Open);

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(
            result,
            Ok(BaseResolution::Live {
                verification_error: None,
                ..
            })
        ));
    }

    #[test]
    fn resolve_errors_ref_missing_with_open_pr() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .failing_resolve_ref_for(
                "heist/piece-01",
                GitError::MergeCheck {
                    message: "missing".into(),
                },
            )
            .with_pr_state("heist/piece-01", PrState::Open);

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(
            result,
            Err(ResolveError::RefMissingWithOpenPr { .. })
        ));
    }

    #[test]
    fn resolve_errors_ref_missing_no_pr_when_ref_gone_and_no_pr_exists() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        // Ref does not resolve, and no PR was ever found (FakeGit's default
        // pr_state is `None`).
        let git = FakeGit::new().failing_resolve_ref_for(
            "heist/piece-01",
            GitError::RefResolve {
                ref_spec: "heist/piece-01".into(),
                message: "not found".into(),
            },
        );

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Err(ResolveError::RefMissingNoPr { .. })));
    }

    #[test]
    fn resolve_returns_live_with_verification_error_when_gh_fails_but_ref_exists() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().failing_pr_state_for(
            "heist/piece-01",
            GitError::CommandFailed {
                command: "gh pr list".into(),
                message: "gh not found".into(),
            },
        );

        let result = resolve(Path::new("."), &repo, &git, "foo");

        match result {
            Ok(BaseResolution::Live {
                verification_error, ..
            }) => {
                let message = verification_error.expect("expected a verification error");
                assert!(
                    message.contains("gh not found"),
                    "expected verification_error to mention the underlying failure, got: {}",
                    message
                );
            }
            other => panic!("expected Live with verification_error, got {:?}", {
                match other {
                    Ok(_) => "Ok(non-Live)".to_string(),
                    Err(_) => "Err".to_string(),
                }
            }),
        }
    }

    #[test]
    fn resolve_errors_ambiguous_when_ref_missing_and_gh_unavailable() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .failing_resolve_ref_for(
                "heist/piece-01",
                GitError::MergeCheck {
                    message: "missing".into(),
                },
            )
            .failing_pr_state_for(
                "heist/piece-01",
                GitError::CommandFailed {
                    command: "gh pr list".into(),
                    message: "gh not found".into(),
                },
            );

        let result = resolve(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Err(ResolveError::Ambiguous { .. })));
    }
}
