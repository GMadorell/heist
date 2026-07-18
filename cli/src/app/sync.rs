use crate::app::base::BaseResolution;
use crate::ports::git::{GitError, GitRepository};
use crate::ports::state_repository::StateRepository;
use std::path::Path;

pub enum SyncError {
    Resolve(crate::app::base::ResolveError),
    Abandoned {
        base_ref: String,
    },
    /// The heist has no worktree recorded, so there is nothing safe to sync.
    NotSetUp,
    /// The recorded worktree is checked out on a different branch than the
    /// heist owns (or `HEAD` is detached). Refuse rather than mutate the
    /// wrong branch.
    WrongCheckout {
        expected: String,
        actual: String,
    },
    /// The pre-sync `git fetch origin` failed. Syncing against stale refs
    /// could rebase or merge the wrong thing, so refuse and let the caller
    /// fix the environment and re-run.
    FetchFailed(GitError),
    Git(GitError),
}

/// What `sync` actually did, so the caller can report it.
pub enum SyncAction {
    /// Unstacked heist: rebased onto `origin/<main>` (today's behavior).
    RebasedOntoMain { onto: String },
    /// Stacked on a still-open base: merged that base in.
    MergedBase { base_ref: String },
    /// Stacked on a base whose PR already merged: merged `origin/<main>` in.
    MergedMainBaseMerged { onto: String },
}

pub fn perform(
    repo_root: &Path,
    git: &dyn GitRepository,
    main_branch: &str,
    resolution: &BaseResolution,
) -> Result<SyncAction, SyncError> {
    match resolution {
        BaseResolution::Null => {
            let onto = format!("origin/{}", main_branch);
            git.rebase(repo_root, &onto).map_err(SyncError::Git)?;
            Ok(SyncAction::RebasedOntoMain { onto })
        }
        BaseResolution::Live { base_ref } => {
            git.merge(repo_root, base_ref.as_ref())
                .map_err(SyncError::Git)?;
            Ok(SyncAction::MergedBase {
                base_ref: base_ref.to_string(),
            })
        }
        BaseResolution::Expired { .. } => {
            let onto = format!("origin/{}", main_branch);
            git.merge(repo_root, &onto).map_err(SyncError::Git)?;
            Ok(SyncAction::MergedMainBaseMerged { onto })
        }
        BaseResolution::Abandoned { base_ref } => Err(SyncError::Abandoned {
            base_ref: base_ref.to_string(),
        }),
    }
}

/// Syncs the heist's branch against its base. Operates strictly on the
/// worktree recorded in state, never the caller's current directory, so it is
/// safe to invoke from anywhere.
pub fn sync(
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    slug: &str,
) -> Result<SyncAction, SyncError> {
    if !state_repo.exists(slug) {
        return Err(SyncError::Resolve(crate::app::base::ResolveError::NoState));
    }
    let state = state_repo
        .load(slug)
        .map_err(|e| SyncError::Resolve(crate::app::base::ResolveError::Load(e)))?;

    // A sync mutates a branch; it must run in the heist's own worktree, not
    // whatever checkout the caller happens to stand in. Resolve the worktree
    // from state and confirm it is on the branch we own before touching git.
    let (worktree, branch) = match (state.worktree.as_ref(), state.branch.as_ref()) {
        (Some(worktree), Some(branch)) => (worktree, branch),
        _ => return Err(SyncError::NotSetUp),
    };
    let worktree_path = Path::new(worktree.as_ref());

    match git.current_branch(worktree_path).map_err(SyncError::Git)? {
        Some(current) if current == branch.as_ref() => {}
        other => {
            return Err(SyncError::WrongCheckout {
                expected: branch.to_string(),
                actual: other.unwrap_or_else(|| "(detached HEAD)".to_string()),
            });
        }
    }

    // Refresh `origin/*` so the ancestry pre-check and any rebase/merge onto
    // `origin/<main>` see the remote's current state rather than stale refs.
    // A failed fetch means stale refs, and every downstream decision assumes
    // fresh ones, so it is a hard stop.
    git.fetch(worktree_path, "origin")
        .map_err(SyncError::FetchFailed)?;

    let resolution = crate::app::base::resolve(worktree_path, state_repo, git, slug)
        .map_err(SyncError::Resolve)?;
    let main_branch = git.default_branch(worktree_path);
    perform(worktree_path, git, &main_branch, &resolution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::{DateValue, NonBlankValue};
    use crate::ports::git::PrState;

    /// A fully set-up heist state: worktree + branch recorded, so the sync
    /// guard passes. `base` is left for the caller to set.
    fn set_up_state(slug: &str) -> State {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        let mut state = State::new(slug, today).expect("valid slug");
        state.worktree = Some(NonBlankValue::parse("worktree", "/tmp/wt").expect("valid worktree"));
        state.branch =
            Some(NonBlankValue::parse("branch", &format!("heist/{}", slug)).expect("valid branch"));
        state
    }

    fn git_on_branch(slug: &str) -> FakeGit {
        FakeGit::new()
            .with_default_branch("main")
            .with_current_branch(&format!("heist/{}", slug))
    }

    #[test]
    fn sync_with_null_base_rebases_origin_default() {
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state("foo"));
        let git = git_on_branch("foo");

        let result = sync(&repo, &git, "foo");

        assert!(result.is_ok());
        assert_eq!(git.rebase_calls(), vec!["origin/main".to_string()]);
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_with_live_base_merges_base_ref_not_origin_default() {
        let mut state = set_up_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch("foo").with_pr_state("heist/piece-01", PrState::Open);

        let result = sync(&repo, &git, "foo");

        assert!(result.is_ok());
        assert_eq!(git.merge_calls(), vec!["heist/piece-01".to_string()]);
        assert!(git.rebase_calls().is_empty());
    }

    #[test]
    fn sync_with_expired_base_merges_origin_default() {
        let mut state = set_up_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch("foo").with_pr_state("heist/piece-01", PrState::Merged);

        let result = sync(&repo, &git, "foo");

        assert!(result.is_ok());
        assert_eq!(git.merge_calls(), vec!["origin/main".to_string()]);
        assert!(git.rebase_calls().is_empty());
    }

    #[test]
    fn sync_with_abandoned_base_refuses_without_touching_git() {
        let mut state = set_up_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch("foo").with_pr_state("heist/piece-01", PrState::ClosedUnmerged);

        let result = sync(&repo, &git, "foo");

        assert!(matches!(result, Err(SyncError::Abandoned { .. })));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_halts_without_touching_git_when_base_pr_state_unverifiable() {
        let mut state = set_up_state("foo");
        state.base = Some(NonBlankValue::parse("base", "heist/piece-01").expect("valid base"));

        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch("foo").failing_pr_state_for(
            "heist/piece-01",
            crate::ports::git::GitError::CommandFailed {
                command: "gh pr list".into(),
                message: "gh not found".into(),
            },
        );

        let result = sync(&repo, &git, "foo");

        assert!(matches!(
            result,
            Err(SyncError::Resolve(
                crate::app::base::ResolveError::VerificationFailed { .. }
            ))
        ));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_refuses_when_worktree_on_wrong_branch() {
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state("foo"));
        // Worktree reports being on `main`, not `heist/foo`.
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_current_branch("main");

        let result = sync(&repo, &git, "foo");

        assert!(matches!(result, Err(SyncError::WrongCheckout { .. })));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_refuses_when_head_detached() {
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state("foo"));
        // No current branch configured => detached HEAD.
        let git = FakeGit::new().with_default_branch("main");

        let result = sync(&repo, &git, "foo");

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
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        let state = State::new("foo", today).expect("valid slug");
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let git = git_on_branch("foo");

        let result = sync(&repo, &git, "foo");

        assert!(matches!(result, Err(SyncError::NotSetUp)));
    }

    #[test]
    fn sync_fetches_origin_before_any_rebase_or_merge() {
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state("foo"));
        let git = git_on_branch("foo");

        let result = sync(&repo, &git, "foo");

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
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state("foo"));
        let git = git_on_branch("foo").failing_fetch(GitError::CommandFailed {
            command: "git fetch".into(),
            message: "network down".into(),
        });

        let result = sync(&repo, &git, "foo");

        assert!(matches!(result, Err(SyncError::FetchFailed(_))));
        assert!(git.rebase_calls().is_empty());
        assert!(git.merge_calls().is_empty());
    }

    #[test]
    fn sync_reports_action_taken() {
        let repo = InMemoryStateRepository::new().with_state("foo", set_up_state("foo"));
        let git = git_on_branch("foo");

        let Ok(action) = sync(&repo, &git, "foo") else {
            panic!("sync should succeed");
        };

        assert!(matches!(action, SyncAction::RebasedOntoMain { .. }));
    }
}
