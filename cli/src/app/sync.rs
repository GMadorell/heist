use crate::app::base::{self, BaseResolution, ResolveError};
use crate::domain::error::ValueError;
use crate::domain::value::{RefValue, SlugValue};
use crate::ports::git::{GitError, GitRepository};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

pub enum SyncError {
    Resolve(ResolveError),
    Abandoned { base_ref: String },
    NotSetUp,
    WrongCheckout { expected: String, actual: String },
    FetchFailed(GitError),
    InvalidComposedRef(ValueError),
    Git(GitError),
}

pub enum SyncAction {
    RebasedOntoMain { onto: String },
    MergedBase { base_ref: String },
    MergedMainBaseMerged { onto: String },
}

pub fn sync(
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &SlugValue,
) -> Result<SyncAction, SyncError> {
    if !state_repo.exists(slug) {
        return Err(SyncError::Resolve(ResolveError::NoState));
    }
    let state = state_repo
        .load(slug)
        .map_err(|e| SyncError::Resolve(ResolveError::Load(e)))?;

    let (worktree, branch) = match (state.worktree.as_ref(), state.branch.as_ref()) {
        (Some(worktree), Some(branch)) => (worktree, branch),
        _ => return Err(SyncError::NotSetUp),
    };
    let worktree_path = Path::new(worktree.as_ref());

    // Sync mutates the branch; the worktree must actually have it checked out.
    match git.current_branch(worktree_path).map_err(SyncError::Git)? {
        Some(current) if current == branch.as_ref() => {}
        other => {
            return Err(SyncError::WrongCheckout {
                expected: branch.to_string(),
                actual: other.unwrap_or_else(|| "(detached HEAD)".to_string()),
            });
        }
    }

    // Every downstream decision assumes fresh `origin/*` refs, so a failed
    // fetch is a hard stop.
    git.fetch(worktree_path, "origin")
        .map_err(SyncError::FetchFailed)?;

    let resolution =
        base::resolve(worktree_path, state_repo, git, slug).map_err(SyncError::Resolve)?;
    let main_branch = git.default_branch(worktree_path);
    perform(worktree_path, git, &main_branch, &resolution)
}

pub fn perform(
    repo_root: &Path,
    git: &dyn GitRepository,
    main_branch: &str,
    resolution: &BaseResolution,
) -> Result<SyncAction, SyncError> {
    match resolution {
        BaseResolution::Null => {
            let onto_str = format!("origin/{}", main_branch);
            let onto_ref =
                RefValue::try_from_raw(&onto_str).map_err(SyncError::InvalidComposedRef)?;
            git.rebase(repo_root, &onto_ref).map_err(SyncError::Git)?;
            Ok(SyncAction::RebasedOntoMain { onto: onto_str })
        }
        BaseResolution::Live { base_ref } => {
            let base_ref_value =
                RefValue::try_from_raw(base_ref.as_ref()).map_err(SyncError::InvalidComposedRef)?;
            git.merge(repo_root, &base_ref_value)
                .map_err(SyncError::Git)?;
            Ok(SyncAction::MergedBase {
                base_ref: base_ref.to_string(),
            })
        }
        BaseResolution::Expired { .. } => {
            let onto_str = format!("origin/{}", main_branch);
            let onto_ref =
                RefValue::try_from_raw(&onto_str).map_err(SyncError::InvalidComposedRef)?;
            git.merge(repo_root, &onto_ref).map_err(SyncError::Git)?;
            Ok(SyncAction::MergedMainBaseMerged { onto: onto_str })
        }
        BaseResolution::Abandoned { base_ref } => Err(SyncError::Abandoned {
            base_ref: base_ref.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::testing::valid;
    use crate::ports::git::PrState;

    /// A fully set-up heist state: worktree + branch recorded, so the sync
    /// guard passes. `base` is left for the caller to set.
    fn set_up_state(slug: &SlugValue) -> State {
        let today = valid::date("2026-01-01");
        let mut state = State::new(slug, today).expect("valid slug");
        state.worktree = Some(valid::worktree("/tmp/wt"));
        state.branch = Some(valid::branch(&format!("heist/{}", slug)));
        state
    }

    fn git_on_branch(slug: &SlugValue) -> FakeGit {
        FakeGit::new()
            .with_default_branch("main")
            .with_current_branch(&format!("heist/{}", slug))
    }

    #[test]
    fn sync_with_null_base_rebases_origin_default() {
        let slug = valid::slug("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state(&slug));
        let git = git_on_branch(&slug);

        let result = sync(&repo, &git, &slug);

        assert!(result.is_ok());
        assert_eq!(git.rebase_calls(), vec!["origin/main".to_string()]);
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_with_live_base_merges_base_ref_not_origin_default() {
        let slug = valid::slug("foo");
        let mut state = set_up_state(&slug);
        state.base = Some(valid::base("heist/piece-01"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch(&slug).with_pr_state("heist/piece-01", PrState::Open);

        let result = sync(&repo, &git, &slug);

        assert!(result.is_ok());
        assert_eq!(git.merge_calls(), vec!["heist/piece-01".to_string()]);
        assert!(git.rebase_calls().is_empty());
    }

    #[test]
    fn sync_with_expired_base_merges_origin_default() {
        let slug = valid::slug("foo");
        let mut state = set_up_state(&slug);
        state.base = Some(valid::base("heist/piece-01"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch(&slug).with_pr_state("heist/piece-01", PrState::Merged);

        let result = sync(&repo, &git, &slug);

        assert!(result.is_ok());
        assert_eq!(git.merge_calls(), vec!["origin/main".to_string()]);
        assert!(git.rebase_calls().is_empty());
    }

    #[test]
    fn sync_with_abandoned_base_refuses_without_touching_git() {
        let slug = valid::slug("foo");
        let mut state = set_up_state(&slug);
        state.base = Some(valid::base("heist/piece-01"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch(&slug).with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let result = sync(&repo, &git, &slug);

        assert!(matches!(result, Err(SyncError::Abandoned { .. })));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_halts_without_touching_git_when_base_pr_state_unverifiable() {
        let slug = valid::slug("foo");
        let mut state = set_up_state(&slug);
        state.base = Some(valid::base("heist/piece-01"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch(&slug).failing_pr_state_for(
            "heist/piece-01",
            GitError::CommandFailed {
                command: "gh pr list".into(),
                message: "gh not found".into(),
            },
        );

        let result = sync(&repo, &git, &slug);

        assert!(matches!(
            result,
            Err(SyncError::Resolve(ResolveError::VerificationFailed { .. }))
        ));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_refuses_when_worktree_on_wrong_branch() {
        let slug = valid::slug("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state(&slug));
        // Worktree reports being on `main`, not `heist/foo`.
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_current_branch("main");

        let result = sync(&repo, &git, &slug);

        assert!(matches!(result, Err(SyncError::WrongCheckout { .. })));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_refuses_when_head_detached() {
        let slug = valid::slug("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state(&slug));
        // No current branch configured => detached HEAD.
        let git = FakeGit::new().with_default_branch("main");

        let result = sync(&repo, &git, &slug);

        match result {
            Err(SyncError::WrongCheckout { actual, .. }) => {
                assert_eq!(actual, "(detached HEAD)");
            }
            _ => panic!("expected WrongCheckout for detached HEAD"),
        }
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_without_worktree_is_not_set_up() {
        let slug = valid::slug("foo");
        let today = valid::date("2026-01-01");
        let state = State::new(&slug, today).expect("valid slug");
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch(&slug);

        let result = sync(&repo, &git, &slug);

        assert!(matches!(result, Err(SyncError::NotSetUp)));
    }

    #[test]
    fn sync_fetches_origin_before_any_rebase_or_merge() {
        let slug = valid::slug("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state(&slug));
        let git = git_on_branch(&slug);

        let result = sync(&repo, &git, &slug);

        assert!(result.is_ok());
        assert_eq!(git.fetch_calls(), vec!["origin".to_string()]);
        let log = git.call_log();
        let fetch_pos = log.iter().position(|c| c == "fetch");
        let rebase_pos = log.iter().position(|c| c == "rebase");
        assert!(
            fetch_pos < rebase_pos,
            "fetch must be recorded before rebase, got {:?}",
            log
        );
    }

    #[test]
    fn sync_fails_without_touching_git_when_fetch_fails() {
        let slug = valid::slug("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state(&slug));
        let git = git_on_branch(&slug).failing_fetch(GitError::CommandFailed {
            command: "git fetch".into(),
            message: "network down".into(),
        });

        let result = sync(&repo, &git, &slug);

        assert!(matches!(result, Err(SyncError::FetchFailed(_))));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_reports_action_taken() {
        let slug = valid::slug("foo");
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state(&slug));
        let git = git_on_branch(&slug);

        let Ok(action) = sync(&repo, &git, &slug) else {
            panic!("sync should succeed");
        };

        assert!(matches!(action, SyncAction::RebasedOntoMain { .. }));
    }
}
