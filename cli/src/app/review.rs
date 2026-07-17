use crate::domain::error::StateError;
use crate::domain::language;
use crate::domain::review::{self, Lane};
use crate::ports::git::{GitError, GitRepository};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

#[derive(Debug)]
pub enum SelectError {
    NoState,
    NoBranch,
    Load(StateError),
    Git(GitError),
}

pub fn select(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &str,
) -> Result<Vec<Lane>, SelectError> {
    if !state_repo.exists(slug) {
        return Err(SelectError::NoState);
    }
    let state = state_repo.load(slug).map_err(SelectError::Load)?;
    let branch = state.branch.ok_or(SelectError::NoBranch)?;

    let main_branch = git.default_branch(repo_root);
    let paths = git
        .changed_paths(repo_root, &main_branch, branch.as_ref())
        .map_err(SelectError::Git)?;

    let language_types: Vec<_> = paths.iter().map(|p| language::classify(p)).collect();
    Ok(review::select_lanes(&language_types))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::{DateValue, NonBlankValue};
    use std::path::Path;

    fn fixed_date() -> DateValue {
        DateValue::parse("today", "2026-01-01").expect("valid date")
    }

    #[test]
    fn missing_state_is_no_state_error() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new();

        let err =
            select(Path::new("."), &repo, &git, "foo").expect_err("should fail without state");
        assert!(matches!(err, SelectError::NoState));
    }

    #[test]
    fn state_without_branch_is_no_branch_error() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new("foo", fixed_date()).expect("valid slug"));
        let git = FakeGit::new();

        let err =
            select(Path::new("."), &repo, &git, "foo").expect_err("should fail without branch");
        assert!(matches!(err, SelectError::NoBranch));
    }

    #[test]
    fn no_changed_files_selects_intent_only() {
        let mut state = State::new("foo", fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_default_branch("main");

        let lanes = select(Path::new("."), &repo, &git, "foo").expect("select should succeed");
        assert_eq!(lanes, vec![Lane::Intent]);
    }

    #[test]
    fn rust_file_change_selects_all_gated_lanes() {
        let mut state = State::new("foo", fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_changed_paths(&["src/lib.rs"]);

        let lanes = select(Path::new("."), &repo, &git, "foo").expect("select should succeed");
        assert_eq!(
            lanes,
            vec![
                Lane::Intent,
                Lane::Coverage,
                Lane::Quality,
                Lane::Simplicity,
                Lane::Rust
            ]
        );
    }

    #[test]
    fn git_failure_propagates_as_git_error() {
        let mut state = State::new("foo", fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_changed_paths(crate::ports::git::GitError::Diff {
                message: "bad ref".into(),
            });

        let err =
            select(Path::new("."), &repo, &git, "foo").expect_err("should propagate git error");
        assert!(matches!(err, SelectError::Git(_)));
    }
}
