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
    fs.ensure_worktrees_ignored(repo_root).map_err(AddError::Fs)?;

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

    // Remote branch deletion is intentionally out of scope: the branch is
    // often never pushed, or GitHub's auto-delete-on-merge already handled
    // it, and failing there would strand state.json short of "done".

    let mut state = state_repo.load(slug).map_err(RemoveError::Load)?;
    state.stage = Stage::Done;
    state.updated = clock.today();
    state_repo.save(slug, &state).map_err(RemoveError::Save)
}
