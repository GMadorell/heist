use crate::domain::error::{StateError, ValueError};
use crate::domain::value::{NonBlankValue, RefValue, SlugValue};
use crate::ports::git::{GitRepository, PrState};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

pub enum BaseResolution {
    Null,
    Live { base_ref: NonBlankValue },
    Expired { base_ref: NonBlankValue },
    Abandoned { base_ref: NonBlankValue },
}

pub enum ResolveError {
    NoState,
    Load(StateError),
    InvalidStoredBase(ValueError),
    RefMissingWithOpenPr { base_ref: String },
    RefMissingNoPr { base_ref: String },
    Ambiguous { base_ref: String },
    VerificationFailed { base_ref: String, message: String },
}

pub fn resolve(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &SlugValue,
) -> Result<BaseResolution, ResolveError> {
    if !state_repo.exists(slug) {
        return Err(ResolveError::NoState);
    }

    let state = state_repo.load(slug).map_err(ResolveError::Load)?;

    let Some(base_value) = state.base else {
        return Ok(BaseResolution::Null);
    };
    let base_ref =
        RefValue::try_from_raw(base_value.as_ref()).map_err(ResolveError::InvalidStoredBase)?;
    let main_branch = git.default_branch(repo_root);

    let ref_exists = git.resolve_ref(repo_root, &base_ref).is_ok();

    if ref_exists {
        let origin_main = RefValue::try_from_raw(&format!("origin/{}", main_branch))
            .map_err(ResolveError::InvalidStoredBase)?;
        let ancestry = git.is_ancestor(repo_root, &base_ref, &origin_main);
        if matches!(ancestry, Ok(true)) {
            return Ok(BaseResolution::Expired {
                base_ref: base_value,
            });
        }
    }

    match git.pr_state(repo_root, &base_ref) {
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
                })
            } else {
                Err(ResolveError::RefMissingWithOpenPr {
                    base_ref: base_ref.to_string(),
                })
            }
        }
        Ok(PrState::None) => {
            if ref_exists {
                Ok(BaseResolution::Live {
                    base_ref: base_value,
                })
            } else {
                Err(ResolveError::RefMissingNoPr {
                    base_ref: base_ref.to_string(),
                })
            }
        }
        Err(e) => {
            if ref_exists {
                Err(ResolveError::VerificationFailed {
                    base_ref: base_ref.to_string(),
                    message: e.to_string(),
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

    fn test_state(slug: &SlugValue) -> State {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        State::new(slug, today).expect("valid slug")
    }

    #[test]
    fn resolve_skips_ancestry_check_when_ref_missing_and_asks_gh_directly() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
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

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Ok(BaseResolution::Expired { .. })));
        assert_eq!(git.is_ancestor_call_count(), 0);
    }

    #[test]
    fn resolve_returns_null_when_state_base_is_none() {
        let state = test_state(&SlugValue::parse("foo").expect("valid slug"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new();

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Ok(BaseResolution::Null)));
    }

    #[test]
    fn resolve_returns_expired_when_ref_resolves_and_is_ancestor_of_main() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_ancestor("heist/piece-01", "origin/main");

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Ok(BaseResolution::Expired { .. })));
    }

    #[test]
    fn resolve_returns_expired_when_gh_reports_merged() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::Merged);

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Ok(BaseResolution::Expired { .. })));
    }

    #[test]
    fn resolve_returns_abandoned_when_gh_reports_closed_unmerged() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Ok(BaseResolution::Abandoned { .. })));
    }

    #[test]
    fn resolve_returns_live_when_ref_resolves_and_gh_reports_open() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::Open);

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Ok(BaseResolution::Live { .. })));
    }

    #[test]
    fn resolve_errors_ref_missing_with_open_pr() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
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

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(
            result,
            Err(ResolveError::RefMissingWithOpenPr { .. })
        ));
    }

    #[test]
    fn resolve_errors_ref_missing_no_pr_when_ref_gone_and_no_pr_exists() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
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

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Err(ResolveError::RefMissingNoPr { .. })));
    }

    #[test]
    fn resolve_errors_verification_failed_when_gh_fails_even_if_ref_exists() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().failing_pr_state_for(
            "heist/piece-01",
            GitError::CommandFailed {
                command: "gh pr list".into(),
                message: "gh not found".into(),
            },
        );

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        match result {
            Err(ResolveError::VerificationFailed { base_ref, message }) => {
                assert_eq!(base_ref, "heist/piece-01");
                assert!(
                    message.contains("gh not found"),
                    "expected message to mention the underlying failure, got: {}",
                    message
                );
            }
            _ => panic!("expected VerificationFailed"),
        }
    }

    #[test]
    fn resolve_errors_ambiguous_when_ref_missing_and_gh_unavailable() {
        let mut state = test_state(&SlugValue::parse("foo").expect("valid slug"));
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

        let result = resolve(
            Path::new("."),
            &repo,
            &git,
            &SlugValue::parse("foo").expect("valid slug"),
        );

        assert!(matches!(result, Err(ResolveError::Ambiguous { .. })));
    }
}
