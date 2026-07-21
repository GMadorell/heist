use crate::domain::error::StateError;
use crate::domain::language;
use crate::domain::review::{self, Lane};
use crate::domain::value::{RefValue, SlugValue};
use crate::ports::git::{GitError, GitRepository};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

#[derive(Debug)]
pub enum SelectError {
    NoState,
    NoBranch,
    InvalidSlug(crate::domain::error::ValueError),
    Load(StateError),
    NoRemoteDefault(GitError),
    Git(GitError),
}

pub fn select(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &SlugValue,
) -> Result<Vec<Lane>, SelectError> {
    if !state_repo.exists(slug) {
        return Err(SelectError::NoState);
    }
    let state = state_repo.load(slug).map_err(SelectError::Load)?;
    let _branch_recorded = state.branch.ok_or(SelectError::NoBranch)?;

    let branch = crate::domain::worktree::branch_name(slug)
        .map_err(SelectError::InvalidSlug)?;

    let main_branch = git.default_branch(repo_root);
    git.remote_default_resolves(repo_root, &main_branch)
        .map_err(SelectError::NoRemoteDefault)?;
    let paths = git
        .changed_paths(
            repo_root,
            &main_branch,
            &RefValue::from(branch.clone()),
        )
        .map_err(SelectError::Git)?;

    let language_types: Vec<_> = paths
        .iter()
        .map(|p| {
            let content = git
                .read_file_at(repo_root, &RefValue::from(branch.clone()), p)
                .unwrap_or(None);
            language::classify(p, content.as_deref())
        })
        .collect();
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
            select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug")).expect_err("should fail without state");
        assert!(matches!(err, SelectError::NoState));
    }

    #[test]
    fn state_without_branch_is_no_branch_error() {
        let repo = InMemoryStateRepository::new()
            .with_state("foo", State::new(&SlugValue::parse("foo").expect("valid slug"), fixed_date()).expect("valid slug"));
        let git = FakeGit::new();

        let err =
            select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug")).expect_err("should fail without branch");
        assert!(matches!(err, SelectError::NoBranch));
    }

    #[test]
    fn no_changed_files_selects_intent_only() {
        let mut state = State::new(&SlugValue::parse("foo").expect("valid slug"), fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_default_branch("main");

        let lanes = select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug")).expect("select should succeed");
        assert_eq!(lanes, vec![Lane::Intent]);
    }

    #[test]
    fn rust_file_change_selects_all_gated_lanes() {
        let mut state = State::new(&SlugValue::parse("foo").expect("valid slug"), fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_changed_paths(&["src/lib.rs"]);

        let lanes = select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug")).expect("select should succeed");
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
    fn corrupt_state_is_load_error() {
        let repo = InMemoryStateRepository::new()
            .with_load_error("foo", StateError::Unparseable(invalid_json_error()));
        let git = FakeGit::new();

        let err =
            select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug")).expect_err("should fail to load state");
        assert!(matches!(err, SelectError::Load(StateError::Unparseable(_))));
    }

    fn invalid_json_error() -> serde_json::Error {
        serde_json::from_str::<State>("not json").expect_err("should fail to parse")
    }

    #[test]
    fn unresolvable_remote_default_is_no_remote_default_error() {
        let mut state = State::new(&SlugValue::parse("foo").expect("valid slug"), fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_remote_default_resolve(crate::ports::git::GitError::MergeCheck {
                message: "no origin/main".into(),
            });

        let err = select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug"))
            .expect_err("should fail when origin/<default> doesn't resolve");
        assert!(matches!(err, SelectError::NoRemoteDefault(_)));
    }

    #[test]
    fn git_failure_propagates_as_git_error() {
        let mut state = State::new(&SlugValue::parse("foo").expect("valid slug"), fixed_date()).expect("valid slug");
        state.branch = Some(NonBlankValue::parse("branch", "heist/foo").expect("valid branch"));
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_changed_paths(crate::ports::git::GitError::Diff {
                message: "bad ref".into(),
            });

        let err =
            select(Path::new("."), &repo, &git, &SlugValue::parse("foo").expect("valid slug")).expect_err("should propagate git error");
        assert!(matches!(err, SelectError::Git(_)));
    }
}
