use crate::ports::git::{GitError, GitRepository, WorktreeInfo};
use std::path::Path;

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

    fn remote_default_resolves(&self, repo_root: &Path, main_branch: &str) -> Result<(), GitError> {
        let resolve = || -> Result<(), git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            repo.revparse_single(&format!("origin/{}", main_branch))?;
            Ok(())
        };
        resolve().map_err(|e| GitError::MergeCheck {
            message: e.to_string(),
        })
    }

    fn list_worktrees(&self, repo_root: &Path) -> Result<Vec<WorktreeInfo>, GitError> {
        let list = || -> Result<Vec<WorktreeInfo>, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let names = repo.worktrees()?;
            let mut infos = Vec::new();
            for name in names.iter().flatten().flatten() {
                let worktree = repo.find_worktree(name)?;
                let path = worktree.path().to_path_buf();
                let branch = if let Ok(wt_repo) = git2::Repository::open_from_worktree(&worktree) {
                    wt_repo
                        .head()
                        .ok()
                        .and_then(|head| head.shorthand().ok().map(str::to_string))
                } else {
                    None
                };
                infos.push(WorktreeInfo { path, branch });
            }
            Ok(infos)
        };
        list().map_err(|e| GitError::CommandFailed {
            command: "git worktree list".to_string(),
            message: e.to_string(),
        })
    }
}
