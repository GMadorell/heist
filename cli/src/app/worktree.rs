use crate::domain::error::{FieldError, StateError};
use crate::domain::state::Stage;
use crate::domain::value::NonBlankValue;
use crate::domain::worktree;
use crate::ports::clock::Clock;
use crate::ports::git::{GitError, GitRepository};
use crate::ports::state_repository::StateRepository;
use crate::ports::worktree_fs::WorktreeFs;
use std::path::Path;

pub enum AddError {
    NoState,
    Naming(FieldError),
    Fs(std::io::Error),
    Git(GitError),
    Load(StateError),
    Save(StateError),
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
    slug: &str,
) -> Result<NonBlankValue, AddError> {
    if !state_repo.exists(slug) {
        return Err(AddError::NoState);
    }

    let main_branch = git.default_branch(repo_root);
    fs.ensure_worktrees_ignored(repo_root)
        .map_err(AddError::Fs)?;

    let worktree_path = worktree::worktree_path(repo_root, slug);
    let branch = worktree::branch_name(slug).map_err(AddError::Naming)?;

    if !git.worktree_exists(repo_root, slug) {
        git.add_worktree(
            repo_root,
            &worktree_path,
            branch.as_ref(),
            &format!("origin/{}", main_branch),
        )
        .map_err(AddError::Git)?;
    }

    fs.link_heist_dir(repo_root, &worktree_path, slug)
        .map_err(AddError::Fs)?;

    let worktree_absolute = fs.canonicalize(&worktree_path).map_err(AddError::Fs)?;
    let worktree_value = NonBlankValue::parse("worktree", &worktree_absolute.to_string_lossy())
        .map_err(AddError::Naming)?;

    let mut state = state_repo.load(slug).map_err(AddError::Load)?;
    state.worktree = Some(worktree_value.clone());
    state.branch = Some(branch);
    state.updated = clock.today();
    state_repo.save(slug, &state).map_err(AddError::Save)?;

    Ok(worktree_value)
}

pub enum RemoveError {
    NoState,
    Naming(FieldError),
    Git(GitError),
    NotMerged { branch: String, main_branch: String },
    Load(StateError),
    Save(StateError),
}

pub fn remove(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    clock: &dyn Clock,
    slug: &str,
) -> Result<(), RemoveError> {
    if !state_repo.exists(slug) {
        return Err(RemoveError::NoState);
    }

    let main_branch = git.default_branch(repo_root);
    let branch = worktree::branch_name(slug).map_err(RemoveError::Naming)?;

    match git.is_branch_merged(repo_root, branch.as_ref(), &main_branch) {
        Ok(true) => {}
        Ok(false) => {
            return Err(RemoveError::NotMerged {
                branch: branch.to_string(),
                main_branch,
            });
        }
        Err(e) => return Err(RemoveError::Git(e)),
    }

    let worktree_path = worktree::worktree_path(repo_root, slug);
    git.remove_worktree(repo_root, &worktree_path)
        .map_err(RemoveError::Git)?;
    git.delete_branch(repo_root, branch.as_ref())
        .map_err(RemoveError::Git)?;

    // Remote branch deletion is intentionally out of scope: rely
    // on GH auto-delete after merge

    let mut state = state_repo.load(slug).map_err(RemoveError::Load)?;
    state.stage = Stage::Done;
    state.updated = clock.today();
    state_repo.save(slug, &state).map_err(RemoveError::Save)
}

use crate::domain::worktree::HeistWorktree;
use crate::domain::value::SlugValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupOutcome {
    Removed(SlugValue),
    Skipped(SlugValue),
    WouldRemove(SlugValue),
    Failed(SlugValue, String),
}

impl CleanupOutcome {
    fn slug(&self) -> &SlugValue {
        match self {
            CleanupOutcome::Removed(s)
            | CleanupOutcome::Skipped(s)
            | CleanupOutcome::WouldRemove(s)
            | CleanupOutcome::Failed(s, _) => s,
        }
    }
}

#[derive(Debug)]
pub enum CleanupError {
    Fs(std::io::Error),
    Git(GitError),
}

#[allow(clippy::too_many_arguments)]
pub fn cleanup(
    repo_root: &Path,
    state_repo: &dyn StateRepository,
    git: &dyn GitRepository,
    fs: &dyn WorktreeFs,
    clock: &dyn Clock,
    dry_run: bool,
) -> Result<Vec<CleanupOutcome>, CleanupError> {
    let canonical_repo_root = fs.canonicalize(repo_root).map_err(CleanupError::Fs)?;
    let main_branch = git.default_branch(repo_root);

    git.is_branch_merged(repo_root, &main_branch, &main_branch)
        .map_err(CleanupError::Git)?;

    let infos = git.list_worktrees(repo_root).map_err(CleanupError::Git)?;

    let mut outcomes = Vec::new();
    for info in infos {
        let Some(hw) = HeistWorktree::try_from_parts(&info.path, info.branch.as_deref(), &canonical_repo_root) else {
            continue;
        };

        match git.is_branch_merged(repo_root, hw.branch.as_ref(), &main_branch) {
            Ok(true) => {}
            Ok(false) => {
                outcomes.push(CleanupOutcome::Skipped(hw.slug));
                continue;
            }
            Err(e) => {
                outcomes.push(CleanupOutcome::Failed(hw.slug, e.to_string()));
                continue;
            }
        }

        if dry_run {
            outcomes.push(CleanupOutcome::WouldRemove(hw.slug));
            continue;
        }

        if let Err(e) = git.remove_worktree(repo_root, &hw.path) {
            outcomes.push(CleanupOutcome::Failed(hw.slug, e.to_string()));
            continue;
        }

        if let Err(e) = git.delete_branch(repo_root, hw.branch.as_ref()) {
            outcomes.push(CleanupOutcome::Failed(
                hw.slug,
                format!(
                    "worktree removed but branch {} not deleted: {}",
                    hw.branch, e
                ),
            ));
            continue;
        }

        if state_repo.exists(hw.slug.as_ref()) {
            match state_repo.load(hw.slug.as_ref()) {
                Ok(mut state) => {
                    state.stage = Stage::Done;
                    state.updated = clock.today();
                    if let Err(e) = state_repo.save(hw.slug.as_ref(), &state) {
                        outcomes.push(CleanupOutcome::Failed(hw.slug, e.to_string()));
                        continue;
                    }
                }
                Err(e) => {
                    outcomes.push(CleanupOutcome::Failed(hw.slug, e.to_string()));
                    continue;
                }
            }
        }

        outcomes.push(CleanupOutcome::Removed(hw.slug));
    }

    outcomes.sort_by(|a, b| a.slug().as_ref().cmp(b.slug().as_ref()));
    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FakeGit, FakeWorktreeFs, FixedClock, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::DateValue;
    use std::path::Path;

    fn fixed_clock() -> FixedClock {
        FixedClock(DateValue::parse("today", "2026-01-01").expect("valid date"))
    }

    #[test]
    fn cleanup_returns_empty_outcomes_when_no_worktrees() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new().with_default_branch("main");

        let outcomes = cleanup(
            Path::new("."),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert!(outcomes.is_empty());
    }

    #[test]
    fn cleanup_aborts_when_origin_default_unresolvable() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .failing_merge_check(GitError::MergeCheck {
                message: "cannot find remote ref origin/main".into(),
            });

        let result = cleanup(Path::new("."), &repo, &git, &FakeWorktreeFs, &fixed_clock(), false);

        assert!(matches!(result, Err(CleanupError::Git(_))));
    }

    #[test]
    fn cleanup_ignores_non_heist_owned_worktree() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_worktree_info("scratch", "/repo/.worktrees/scratch", Some("some-other-branch"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert!(outcomes.is_empty());
    }

    #[test]
    fn cleanup_skips_unmerged_heist_owned_worktree() {
        let repo = InMemoryStateRepository::new();
        // No merged branch configured, so heist/foo is treated as unmerged.
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_worktree_info("foo", "/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Skipped(SlugValue::parse("foo").expect("valid slug"))]
        );
        assert!(git.removed_worktree_paths().is_empty());
        assert!(git.deleted_branch_names().is_empty());
    }

    #[test]
    fn cleanup_removes_merged_heist_owned_worktree() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("foo", "/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Removed(SlugValue::parse("foo").expect("valid slug"))]
        );
        assert_eq!(
            git.removed_worktree_paths(),
            vec![std::path::PathBuf::from("/repo/.worktrees/foo")]
        );
        assert_eq!(git.deleted_branch_names(), vec!["heist/foo".to_string()]);
    }

    #[test]
    fn cleanup_dry_run_previews_without_mutating() {
        let repo = InMemoryStateRepository::new();
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("foo", "/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            true,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::WouldRemove(SlugValue::parse("foo").expect("valid slug"))]
        );
        assert!(git.removed_worktree_paths().is_empty());
        assert!(git.deleted_branch_names().is_empty());
    }

    #[test]
    fn cleanup_marks_existing_state_done() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new("foo", DateValue::parse("today", "2025-01-01").expect("valid date"))
                .expect("valid slug"),
        );
        let git = FakeGit::new()
            .with_default_branch("main")
            .with_merged_branch("heist/foo")
            .with_worktree_info("foo", "/repo/.worktrees/foo", Some("heist/foo"));

        let outcomes = cleanup(
            Path::new("/repo"),
            &repo,
            &git,
            &FakeWorktreeFs,
            &fixed_clock(),
            false,
        )
        .expect("cleanup should succeed");

        assert_eq!(
            outcomes,
            vec![CleanupOutcome::Removed(SlugValue::parse("foo").expect("valid slug"))]
        );
        let state = repo.get("foo").expect("state should still exist");
        assert_eq!(state.stage, Stage::Done);
        assert_eq!(state.updated, DateValue::parse("today", "2026-01-01").expect("valid date"));
    }
}
