use crate::ports::git::{GitError, GitRepository, MergeCheck, PrState, WorktreeInfo};
use std::path::{Path, PathBuf};

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
    ) -> Result<MergeCheck, GitError> {
        let merged = || -> Result<bool, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let branch_oid = repo.revparse_single(branch)?.id();
            let main_oid = repo.revparse_single(&format!("origin/{}", into))?.id();

            if branch_oid == main_oid {
                return Ok(true);
            }
            repo.graph_descendant_of(main_oid, branch_oid)
        };
        match merged() {
            Ok(true) => Ok(MergeCheck::Merged),
            // Ancestry check misses squash/rebase merges: GitHub creates a
            // new commit rather than reusing the branch's commits, so the
            // branch tip is never reachable from `into` even though the PR
            // landed. Ask the GitHub API as a fallback.
            Ok(false) => Ok(match is_pr_merged_on_github(repo_root, branch) {
                Ok(true) => MergeCheck::Merged,
                Ok(false) => MergeCheck::NotMerged {
                    verification_error: None,
                },
                Err(message) => MergeCheck::NotMerged {
                    verification_error: Some(message),
                },
            }),
            Err(e) => Err(GitError::MergeCheck {
                message: e.to_string(),
            }),
        }
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

    fn resolve_ref(&self, repo_root: &Path, ref_spec: &str) -> Result<(), GitError> {
        let resolve = || -> Result<(), git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            repo.revparse_single(ref_spec)?;
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

    fn changed_paths(
        &self,
        repo_root: &Path,
        base_branch: &str,
        head_ref: &str,
    ) -> Result<Vec<PathBuf>, GitError> {
        let diff_paths = || -> Result<Vec<PathBuf>, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let base_oid = repo
                .revparse_single(&format!("origin/{}", base_branch))?
                .id();
            let head_oid = repo.revparse_single(head_ref)?.id();
            let merge_base_oid = repo.merge_base(base_oid, head_oid)?;
            let base_tree = repo.find_commit(merge_base_oid)?.tree()?;
            let head_tree = repo.find_commit(head_oid)?.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)?;

            let mut paths = Vec::new();
            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path())
                    {
                        paths.push(path.to_path_buf());
                    }
                    true
                },
                None,
                None,
                None,
            )?;

            paths.sort();
            paths.dedup();
            Ok(paths)
        };
        diff_paths().map_err(|e| GitError::Diff {
            message: e.to_string(),
        })
    }

    fn read_file_at(
        &self,
        repo_root: &Path,
        rev: &str,
        path: &Path,
    ) -> Result<Option<String>, GitError> {
        let read = || -> Result<Option<String>, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let commit_oid = repo.revparse_single(rev)?.id();
            let tree = repo.find_commit(commit_oid)?.tree()?;
            let Ok(entry) = tree.get_path(path) else {
                return Ok(None);
            };
            let Ok(blob) = entry.to_object(&repo)?.into_blob() else {
                return Ok(None);
            };
            Ok(std::str::from_utf8(blob.content()).ok().map(str::to_string))
        };
        read().map_err(|e| GitError::Diff {
            message: e.to_string(),
        })
    }

    fn is_ancestor(
        &self,
        repo_root: &Path,
        ancestor_ref: &str,
        descendant_ref: &str,
    ) -> Result<bool, GitError> {
        let check = || -> Result<bool, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let ancestor_oid = repo.revparse_single(ancestor_ref)?.id();
            let descendant_oid = repo.revparse_single(descendant_ref)?.id();

            if ancestor_oid == descendant_oid {
                return Ok(true);
            }
            repo.graph_descendant_of(descendant_oid, ancestor_oid)
        };
        check().map_err(|e| GitError::MergeCheck {
            message: e.to_string(),
        })
    }

    fn pr_state(&self, repo_root: &Path, branch: &str) -> Result<PrState, GitError> {
        let output = std::process::Command::new("gh")
            .current_dir(repo_root)
            .args([
                "pr",
                "list",
                "--head",
                branch,
                "--state",
                "all",
                "--json",
                "state,mergedAt",
                "--limit",
                "1",
            ])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "gh pr list".into(),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(GitError::CommandFailed {
                command: "gh pr list".into(),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| GitError::CommandFailed {
                command: "gh pr list".into(),
                message: format!("failed to parse gh output: {}", e),
            })?;

        let arr = value.as_array().ok_or_else(|| GitError::CommandFailed {
            command: "gh pr list".into(),
            message: "expected JSON array".to_string(),
        })?;

        if arr.is_empty() {
            return Ok(PrState::None);
        }

        let first = &arr[0];
        let merged_at = first.get("mergedAt");
        if merged_at.is_some() && merged_at != Some(&serde_json::Value::Null) {
            return Ok(PrState::Merged);
        }

        let state = first.get("state").and_then(|v| v.as_str()).unwrap_or("");

        if state == "OPEN" {
            Ok(PrState::Open)
        } else {
            Ok(PrState::ClosedUnmerged)
        }
    }
}

fn is_pr_merged_on_github(repo_root: &Path, branch: &str) -> Result<bool, String> {
    let output = std::process::Command::new("gh")
        .current_dir(repo_root)
        .args([
            "pr", "list", "--head", branch, "--state", "merged", "--json", "number", "--limit", "1",
        ])
        .output()
        .map_err(|e| format!("failed to run gh: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse gh output: {}", e))?;
    Ok(value.as_array().map(|arr| !arr.is_empty()).unwrap_or(false))
}
