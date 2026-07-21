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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrState {
    None,
    Open,
    Merged,
    ClosedUnmerged,
}

pub trait GitRepository {
    fn default_branch(&self, repo_root: &Path) -> String;

    fn current_branch(&self, repo_root: &Path) -> Result<Option<String>, GitError>;

    fn fetch(&self, repo_root: &Path, remote: &str) -> Result<(), GitError>;

    fn is_branch_merged(
        &self,
        repo_root: &Path,
        branch: &str,
        into: &str,
    ) -> Result<MergeCheck, GitError>;

    fn delete_branch(&self, repo_root: &Path, branch: &str) -> Result<(), GitError>;

    fn worktree_exists(&self, repo_root: &Path, slug: &str) -> Result<bool, GitError>;

    fn branch_exists(&self, repo_root: &Path, branch: &str) -> Result<bool, GitError>;

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

    /// Resolves `ref_spec` verbatim (no `origin/` prefixing, unlike `remote_default_resolves`),
    /// existence-only check, no ancestry verification.
    fn resolve_ref(&self, repo_root: &Path, ref_spec: &str) -> Result<(), GitError>;

    fn changed_paths(
        &self,
        repo_root: &Path,
        base_branch: &str,
        head_ref: &str,
    ) -> Result<Vec<PathBuf>, GitError>;

    fn read_file_at(
        &self,
        repo_root: &Path,
        rev: &str,
        path: &Path,
    ) -> Result<Option<String>, GitError>;

    /// Returns true if `ancestor_ref` is reachable from `descendant_ref`,
    /// or if they are equal.
    fn is_ancestor(
        &self,
        repo_root: &Path,
        ancestor_ref: &str,
        descendant_ref: &str,
    ) -> Result<bool, GitError>;

    fn pr_state(&self, repo_root: &Path, branch: &str) -> Result<PrState, GitError>;

    fn rebase(&self, repo_root: &Path, onto: &str) -> Result<(), GitError>;

    fn merge(&self, repo_root: &Path, other_ref: &str) -> Result<(), GitError>;
}

#[derive(Debug, Clone)]
pub enum GitError {
    WorktreeAdd { subtype: String, message: String },
    WorktreeRemove { message: String },
    BranchDelete { message: String },
    MergeCheck { message: String },
    RefResolve { ref_spec: String, message: String },
    CommandFailed { command: String, message: String },
    Diff { message: String },
    Rebase { message: String },
    Merge { message: String },
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
            GitError::RefResolve { ref_spec, message } => {
                write!(f, "base ref '{}' not found: {}", ref_spec, message)
            }
            GitError::CommandFailed { command, message } => {
                write!(f, "failed to run {}: {}", command, message)
            }
            GitError::Diff { message } => write!(f, "failed to compute changed paths: {}", message),
            GitError::Rebase { message } => write!(f, "rebase failed: {}", message),
            GitError::Merge { message } => write!(f, "merge failed: {}", message),
        }
    }
}
