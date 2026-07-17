use crate::app::base::BaseResolution;
use crate::ports::git::{GitError, GitRepository};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

pub enum SyncError {
    Resolve(crate::app::base::ResolveError),
    Abandoned { base_ref: String },
    Git(GitError),
}

pub fn perform(
    repo_root: &Path,
    git: &dyn GitRepository,
    main_branch: &str,
    resolution: &BaseResolution,
) -> Result<(), SyncError> {
    match resolution {
        BaseResolution::Null => git
            .rebase(repo_root, &format!("origin/{}", main_branch))
            .map_err(SyncError::Git),
        BaseResolution::Live { base_ref, .. } => git
            .merge(repo_root, base_ref.as_ref())
            .map_err(SyncError::Git),
        BaseResolution::Expired { .. } => git
            .merge(repo_root, &format!("origin/{}", main_branch))
            .map_err(SyncError::Git),
        BaseResolution::Abandoned { base_ref } => Err(SyncError::Abandoned {
            base_ref: base_ref.to_string(),
        }),
    }
}

pub fn sync(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &str,
) -> Result<(), SyncError> {
    let resolution =
        crate::app::base::resolve(repo_root, state_repo, git, slug).map_err(SyncError::Resolve)?;
    let main_branch = git.default_branch(repo_root);
    perform(repo_root, git, &main_branch, &resolution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::{DateValue, NonBlankValue};
    use crate::ports::git::PrState;

    fn test_state(slug: &str) -> State {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        State::new(slug, today).expect("valid slug")
    }

    #[test]
    fn sync_with_null_base_rebases_origin_default() {
        let state = test_state("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_default_branch("main");

        let result = sync(Path::new("."), &repo, &git, "foo");

        assert!(result.is_ok());
        assert_eq!(git.rebase_calls(), vec!["origin/main".to_string()]);
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_with_live_base_merges_base_ref_not_origin_default() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::Open);

        let result = sync(Path::new("."), &repo, &git, "foo");

        assert!(result.is_ok());
        assert_eq!(git.merge_calls(), vec!["heist/piece-01".to_string()]);
        assert!(git.rebase_calls().is_empty());
    }

    #[test]
    fn sync_with_expired_base_merges_origin_default() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_pr_state("heist/piece-01", PrState::Merged);

        let result = sync(Path::new("."), &repo, &git, "foo");

        assert!(result.is_ok());
        assert_eq!(git.merge_calls(), vec!["origin/main".to_string()]);
        assert!(git.rebase_calls().is_empty());
    }

    #[test]
    fn sync_with_abandoned_base_refuses_without_touching_git() {
        let mut state = test_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = FakeGit::new().with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let result = sync(Path::new("."), &repo, &git, "foo");

        assert!(matches!(result, Err(SyncError::Abandoned { .. })));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }
}
