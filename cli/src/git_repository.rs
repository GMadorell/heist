//! Git-access seam for the worktree commands.
//!
//! Command handlers depend on the [`GitRepository`] trait rather than calling
//! `git2`/`std::process::Command` directly, so worktree/branch decisions can be
//! unit-tested against an in-memory [`FakeGit`] without a real repo on disk.

use crate::exitcode::ExitCode;
use std::fmt;
use std::path::Path;

/// Git read/write operations the worktree commands need.
pub trait GitRepository {
    /// The repository's main branch (origin's default, else the current branch).
    fn default_branch(&self, repo_root: &Path) -> String;

    /// Whether `branch` is merged into `origin/<into>`.
    fn is_branch_merged(
        &self,
        repo_root: &Path,
        branch: &str,
        into: &str,
    ) -> Result<bool, GitError>;

    /// Whether a worktree for `slug` is already registered.
    fn worktree_exists(&self, repo_root: &Path, slug: &str) -> bool;

    /// Create a worktree at `path` on a new `branch` from `start_point`.
    fn add_worktree(
        &self,
        repo_root: &Path,
        path: &Path,
        branch: &str,
        start_point: &str,
    ) -> Result<(), GitError>;

    /// Remove the worktree registered at `path`.
    fn remove_worktree(&self, repo_root: &Path, path: &Path) -> Result<(), GitError>;

    /// Delete a local `branch` (safe delete, `git branch -d`).
    fn delete_branch(&self, repo_root: &Path, branch: &str) -> Result<(), GitError>;
}

/// A git operation failure. Every variant maps to [`ExitCode::Git`]; the
/// `Display` string matches the stderr line the CLI printed before this seam.
#[derive(Debug)]
pub enum GitError {
    /// `git worktree add` failed; `subtype` classifies the cause for callers.
    WorktreeAdd { subtype: String, message: String },
    /// `git worktree remove` failed.
    WorktreeRemove { message: String },
    /// `git branch -d` failed.
    BranchDelete { message: String },
    /// The merged-ancestry check itself failed (e.g. a bad ref).
    MergeCheck { message: String },
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
        }
    }
}

/// The real implementation: `git2` for reads, shelling out for the mutating
/// worktree/branch operations.
pub struct RealGit;

impl GitRepository for RealGit {
    /// Detect the repository's main branch, always via git.
    ///
    /// Prefers origin's default branch (`refs/remotes/origin/HEAD`); falls back to
    /// the current branch when there's no remote. The `Main branch:` line in
    /// `validation.md` is for human-facing agents (Cleaner, etc.), not for the
    /// deterministic CLI, which shouldn't parse prose for something git knows.
    fn default_branch(&self, repo_root: &Path) -> String {
        if let Ok(repo) = git2::Repository::open(repo_root) {
            if let Ok(reference) = repo.find_reference("refs/remotes/origin/HEAD") {
                if let Ok(Some(target)) = reference.symbolic_target() {
                    if let Some(name) = target.rsplit('/').next() {
                        if !name.is_empty() {
                            return name.to_string();
                        }
                    }
                }
            }

            // No remote default: fall back to the current branch.
            if let Ok(head) = repo.head() {
                if let Ok(name) = head.shorthand() {
                    return name.to_string();
                }
            }
        }

        String::new()
    }

    /// Whether `branch` is merged into `origin/<into>`.
    ///
    /// True when the branch tip equals the main tip (e.g. after a fast-forward
    /// merge) or is an ancestor of it.
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

    /// Whether a worktree for `slug` is already registered (`.worktrees/<slug>`).
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
            .expect("failed to run git worktree add");

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
            .expect("failed to run git worktree remove");
        if output.status.success() {
            return Ok(());
        }
        Err(GitError::WorktreeRemove {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }

    fn delete_branch(&self, repo_root: &Path, branch: &str) -> Result<(), GitError> {
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "-d", branch])
            .output()
            .expect("failed to run git branch -d");
        if output.status.success() {
            return Ok(());
        }
        Err(GitError::BranchDelete {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// In-memory git for unit tests: a configurable default branch, a set of merged
/// branches, and a set of registered worktree slugs. The three mutating
/// operations can each be configured to fail with a specific [`GitError`].
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
            // Clone the configured error so the fake can fail repeatedly.
            return Err(clone_git_error(err));
        }
        // Register by the branch's slug suffix (`heist/<slug>` -> `<slug>`).
        let slug = branch.rsplit('/').next().unwrap_or(branch);
        self.worktrees.borrow_mut().insert(slug.to_string());
        Ok(())
    }

    fn remove_worktree(&self, _repo_root: &Path, _path: &Path) -> Result<(), GitError> {
        if let Some(err) = &self.remove_error {
            return Err(clone_git_error(err));
        }
        Ok(())
    }

    fn delete_branch(&self, _repo_root: &Path, _branch: &str) -> Result<(), GitError> {
        if let Some(err) = &self.delete_error {
            return Err(clone_git_error(err));
        }
        Ok(())
    }
}

#[cfg(test)]
fn clone_git_error(err: &GitError) -> GitError {
    match err {
        GitError::WorktreeAdd { subtype, message } => GitError::WorktreeAdd {
            subtype: subtype.clone(),
            message: message.clone(),
        },
        GitError::WorktreeRemove { message } => GitError::WorktreeRemove {
            message: message.clone(),
        },
        GitError::BranchDelete { message } => GitError::BranchDelete {
            message: message.clone(),
        },
        GitError::MergeCheck { message } => GitError::MergeCheck {
            message: message.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            // Disable commit signing for these throwaway test repos: if the
            // ambient global git config has commit.gpgsign=true, parallel
            // test threads all invoking gpg-agent concurrently can serialize
            // and occasionally time out, making the test suite flaky.
            .arg("-c")
            .arg("commit.gpgsign=false")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("failed to run git");
        assert!(status.success(), "git {:?} failed", args);
    }

    #[test]
    fn detects_main_branch_via_git() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo_root = temp_dir.path();

        run_git(repo_root, &["init", "-q", "-b", "main"]);
        run_git(repo_root, &["config", "user.email", "test@example.com"]);
        run_git(repo_root, &["config", "user.name", "Test"]);

        fs::write(repo_root.join("README.md"), "hello").expect("failed to write file");
        run_git(repo_root, &["add", "."]);
        run_git(repo_root, &["commit", "-q", "-m", "init"]);

        assert_eq!(RealGit.default_branch(repo_root), "main");
    }

    #[test]
    fn fake_reports_configured_default_branch() {
        let git = FakeGit::new().with_default_branch("trunk");
        assert_eq!(git.default_branch(Path::new(".")), "trunk");
    }

    #[test]
    fn fake_only_reports_configured_branches_as_merged() {
        let git = FakeGit::new().with_merged_branch("heist/foo");
        assert!(git
            .is_branch_merged(Path::new("."), "heist/foo", "main")
            .unwrap());
        assert!(!git
            .is_branch_merged(Path::new("."), "heist/bar", "main")
            .unwrap());
    }

    #[test]
    fn fake_reports_preexisting_worktree() {
        let git = FakeGit::new().with_existing_worktree("foo");
        assert!(git.worktree_exists(Path::new("."), "foo"));
        assert!(!git.worktree_exists(Path::new("."), "bar"));
    }

    #[test]
    fn fake_add_registers_worktree_by_slug() {
        let git = FakeGit::new();
        assert!(!git.worktree_exists(Path::new("."), "foo"));
        git.add_worktree(
            Path::new("."),
            Path::new("/tmp/foo"),
            "heist/foo",
            "origin/main",
        )
        .expect("add should succeed");
        assert!(git.worktree_exists(Path::new("."), "foo"));
    }
}
