use crate::domain::error::StateError;
use crate::domain::value::NonBlankValue;
use crate::ports::git::{GitRepository, PrState};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

pub enum BaseResolution {
    Null,
    Live {
        base_ref: NonBlankValue,
        stale: bool,
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

    if state.base.is_none() {
        return Ok(BaseResolution::Null);
    }

    let base_ref = state.base.as_ref().unwrap().as_ref();
    let main_branch = git.default_branch(repo_root);

    // Check if ref exists
    let ref_exists = git.resolve_ref(repo_root, base_ref).is_ok();

    // Ancestry pre-check (only if ref exists): a fast-forward/non-squash
    // merge may already have landed, in which case there's no need for a
    // `gh` call at all.
    let ancestry_result = if ref_exists {
        let ancestry = git.is_ancestor(repo_root, base_ref, &format!("origin/{}", main_branch));
        if matches!(ancestry, Ok(true)) {
            return Ok(BaseResolution::Expired {
                base_ref: state.base.unwrap(),
            });
        }
        ancestry
    } else {
        Ok(false) // Placeholder, won't be used
    };

    // PR state check
    match git.pr_state(repo_root, base_ref) {
        Ok(PrState::Merged) => Ok(BaseResolution::Expired {
            base_ref: state.base.unwrap(),
        }),
        Ok(PrState::ClosedUnmerged) => Ok(BaseResolution::Abandoned {
            base_ref: state.base.unwrap(),
        }),
        Ok(PrState::Open) | Ok(PrState::None) => {
            if ref_exists {
                let stale = matches!(ancestry_result, Ok(true));
                Ok(BaseResolution::Live {
                    base_ref: state.base.unwrap(),
                    stale,
                })
            } else {
                Err(ResolveError::RefMissingWithOpenPr {
                    base_ref: base_ref.to_string(),
                })
            }
        }
        Err(_) => {
            if ref_exists {
                Ok(BaseResolution::Live {
                    base_ref: state.base.unwrap(),
                    stale: false,
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
            Ok(BaseResolution::Live { stale: false, .. })
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
