use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeCheck {
    Merged,
    /// `verification_error` is `Some` when a secondary check couldn't run
    /// (missing tooling, no auth, no remote configured, etc.) rather than
    /// having actually confirmed the branch is unmerged.
    NotMerged {
        verification_error: Option<String>,
    },
}

pub trait GitRepository {
    fn default_branch(&self, repo_root: &Path) -> String;

    fn is_branch_merged(
        &self,
        repo_root: &Path,
        branch: &str,
        into: &str,
    ) -> Result<MergeCheck, GitError>;

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

    /// Checks that `origin/<main_branch>` resolves to a commit, without
    /// requiring a local branch of the same name to exist.
    fn remote_default_resolves(&self, repo_root: &Path, main_branch: &str) -> Result<(), GitError>;

    /// Changed paths between the merge-base of `origin/<base_branch>` and
    /// `head_ref`, and `head_ref` itself (three-dot semantics).
    fn changed_paths(
        &self,
        repo_root: &Path,
        base_branch: &str,
        head_ref: &str,
    ) -> Result<Vec<PathBuf>, GitError>;

    /// Reads `path` as it exists in `rev`'s tree, straight from the object
    /// database rather than the working directory — correct regardless of
    /// which worktree (if any) `repo_root` has checked out. `Ok(None)` for a
    /// missing path or non-UTF-8 (binary) content; not an error case.
    fn read_file_at(
        &self,
        repo_root: &Path,
        rev: &str,
        path: &Path,
    ) -> Result<Option<String>, GitError>;
}

#[derive(Debug, Clone)]
pub enum GitError {
    WorktreeAdd { subtype: String, message: String },
    WorktreeRemove { message: String },
    BranchDelete { message: String },
    MergeCheck { message: String },
    CommandFailed { command: String, message: String },
    Diff { message: String },
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
            GitError::Diff { message } => write!(f, "failed to compute changed paths: {}", message),
        }
    }
}
