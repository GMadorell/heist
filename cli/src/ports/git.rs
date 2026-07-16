use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
}

pub trait GitRepository {
    fn default_branch(&self, repo_root: &Path) -> String;

    fn is_branch_merged(
        &self,
        repo_root: &Path,
        branch: &str,
        into: &str,
    ) -> Result<bool, GitError>;

    fn delete_branch(&self, repo_root: &Path, branch: &str) -> Result<(), GitError>;

    fn worktree_exists(&self, repo_root: &Path, slug: &str) -> bool;

    fn add_worktree(
        &self,
        repo_root: &Path,
        path: &Path,
        branch: &str,
        start_point: &str,
    ) -> Result<(), GitError>;

    fn remove_worktree(&self, repo_root: &Path, path: &Path) -> Result<(), GitError>;

    fn list_worktrees(&self, repo_root: &Path) -> Result<Vec<WorktreeInfo>, GitError>;
}

#[derive(Debug, Clone)]
pub enum GitError {
    WorktreeAdd { subtype: String, message: String },
    WorktreeRemove { message: String },
    BranchDelete { message: String },
    MergeCheck { message: String },
    CommandFailed { command: String, message: String },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::WorktreeAdd { subtype, message } => write!(f, "{}: {}", subtype, message),
            GitError::WorktreeRemove { message } => {
                write!(f, "worktree-removal-failed: {}", message)
            }
            GitError::BranchDelete { message } => write!(f, "branch-deletion-failed: {}", message),
            GitError::MergeCheck { message } => {
                write!(f, "failed to check merged branches: {}", message)
            }
            GitError::CommandFailed { command, message } => {
                write!(f, "failed to run {}: {}", command, message)
            }
        }
    }
}
