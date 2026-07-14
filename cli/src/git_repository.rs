use crate::exitcode::ExitCode;
use std::fmt;
use std::path::Path;

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
}

#[derive(Debug, Clone)]
pub enum GitError {
    WorktreeAdd { subtype: String, message: String },
    WorktreeRemove { message: String },
    BranchDelete { message: String },
    MergeCheck { message: String },
    CommandFailed { command: String, message: String },
}

impl GitError {
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::Git
    }
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

pub struct RealGit;

impl GitRepository for RealGit {
    fn default_branch(&self, repo_root: &Path) -> String {
        let Ok(repo) = git2::Repository::open(repo_root) else {
            return String::new();
        };

        let remote_default = repo
            .find_reference("refs/remotes/origin/HEAD")
            .ok()
            .and_then(|reference| {
                let target = reference.symbolic_target().ok().flatten()?;
                target.rsplit('/').next().map(str::to_string)
            })
            .filter(|name| !name.is_empty());
        if let Some(name) = remote_default {
            return name;
        }

        // No remote default: fall back to the current branch.
        repo.head()
            .ok()
            .and_then(|head| head.shorthand().ok().map(str::to_string))
            .unwrap_or_default()
    }

    fn is_branch_merged(
        &self,
        repo_root: &Path,
        branch: &str,
        into: &str,
    ) -> Result<bool, GitError> {
        let merged = || -> Result<bool, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let branch_oid = repo.revparse_single(branch)?.id();
            let main_oid = repo.revparse_single(&format!("origin/{}", into))?.id();

            if branch_oid == main_oid {
                return Ok(true);
            }
            repo.graph_descendant_of(main_oid, branch_oid)
        };
        merged().map_err(|e| GitError::MergeCheck {
            message: e.to_string(),
        })
    }

    fn delete_branch(&self, repo_root: &Path, branch: &str) -> Result<(), GitError> {
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "-d", branch])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "git branch -d".to_string(),
                message: e.to_string(),
            })?;
        if output.status.success() {
            return Ok(());
        }
        Err(GitError::BranchDelete {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }

    fn worktree_exists(&self, repo_root: &Path, slug: &str) -> bool {
        if let Ok(repo) = git2::Repository::open(repo_root) {
            if let Ok(worktrees) = repo.worktrees() {
                // iter() yields Result<Option<&str>, _>; flatten twice to reach &str.
                return worktrees
                    .iter()
                    .flatten()
                    .flatten()
                    .any(|name| name == slug);
            }
        }
        false
    }

    fn add_worktree(
        &self,
        repo_root: &Path,
        path: &Path,
        branch: &str,
        start_point: &str,
    ) -> Result<(), GitError> {
        // `git worktree add` is a mutating porcelain command; git2's
        // worktree API is more manual and less battle-tested here, so
        // shelling out stays the pragmatic choice.
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args([
                "worktree",
                "add",
                path.to_string_lossy().as_ref(),
                "-b",
                branch,
                start_point,
            ])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "git worktree add".to_string(),
                message: e.to_string(),
            })?;

        if output.status.success() {
            return Ok(());
        }

        let git_stderr = String::from_utf8_lossy(&output.stderr);
        let subtype = if git_stderr.contains("already exists") {
            "already-exists"
        } else if git_stderr.contains("cannot find remote ref") {
            "origin-unreachable"
        } else if git_stderr.contains("Permission denied") {
            "permission-denied"
        } else {
            "unknown"
        };
        Err(GitError::WorktreeAdd {
            subtype: subtype.to_string(),
            message: git_stderr.trim().to_string(),
        })
    }

    fn remove_worktree(&self, repo_root: &Path, path: &Path) -> Result<(), GitError> {
        // `git worktree remove` / `git branch -d` are mutating porcelain
        // commands; shelling out is more robust than git2's worktree API.
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["worktree", "remove", path.to_string_lossy().as_ref()])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "git worktree remove".to_string(),
                message: e.to_string(),
            })?;
        if output.status.success() {
            return Ok(());
        }
        Err(GitError::WorktreeRemove {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// In-memory git for unit tests
#[cfg(test)]
pub struct FakeGit {
    default_branch: String,
    merged_branches: std::collections::HashSet<String>,
    worktrees: std::cell::RefCell<std::collections::HashSet<String>>,
    add_error: Option<GitError>,
    remove_error: Option<GitError>,
    delete_error: Option<GitError>,
}

#[cfg(test)]
impl FakeGit {
    pub fn new() -> Self {
        FakeGit {
            default_branch: "main".to_string(),
            merged_branches: std::collections::HashSet::new(),
            worktrees: std::cell::RefCell::new(std::collections::HashSet::new()),
            add_error: None,
            remove_error: None,
            delete_error: None,
        }
    }

    pub fn with_default_branch(mut self, branch: &str) -> Self {
        self.default_branch = branch.to_string();
        self
    }

    pub fn with_merged_branch(mut self, branch: &str) -> Self {
        self.merged_branches.insert(branch.to_string());
        self
    }

    pub fn with_existing_worktree(self, slug: &str) -> Self {
        self.worktrees.borrow_mut().insert(slug.to_string());
        self
    }

    pub fn failing_add(mut self, error: GitError) -> Self {
        self.add_error = Some(error);
        self
    }

    pub fn failing_remove(mut self, error: GitError) -> Self {
        self.remove_error = Some(error);
        self
    }

    pub fn failing_delete(mut self, error: GitError) -> Self {
        self.delete_error = Some(error);
        self
    }
}

#[cfg(test)]
impl GitRepository for FakeGit {
    fn default_branch(&self, _repo_root: &Path) -> String {
        self.default_branch.clone()
    }

    fn is_branch_merged(
        &self,
        _repo_root: &Path,
        branch: &str,
        _into: &str,
    ) -> Result<bool, GitError> {
        Ok(self.merged_branches.contains(branch))
    }

    fn worktree_exists(&self, _repo_root: &Path, slug: &str) -> bool {
        self.worktrees.borrow().contains(slug)
    }

    fn add_worktree(
        &self,
        _repo_root: &Path,
        _path: &Path,
        branch: &str,
        _start_point: &str,
    ) -> Result<(), GitError> {
        if let Some(err) = &self.add_error {
            return Err(err.clone());
        }
        // Register by the branch's slug suffix (`heist/<slug>` -> `<slug>`).
        let slug = branch.rsplit('/').next().unwrap_or(branch);
        self.worktrees.borrow_mut().insert(slug.to_string());
        Ok(())
    }

    fn remove_worktree(&self, _repo_root: &Path, _path: &Path) -> Result<(), GitError> {
        if let Some(err) = &self.remove_error {
            return Err(err.clone());
        }
        Ok(())
    }

    fn delete_branch(&self, _repo_root: &Path, _branch: &str) -> Result<(), GitError> {
        if let Some(err) = &self.delete_error {
            return Err(err.clone());
        }
        Ok(())
    }
}
