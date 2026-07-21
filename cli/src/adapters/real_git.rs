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

    fn current_branch(&self, repo_root: &Path) -> Result<Option<String>, GitError> {
        let repo = git2::Repository::open(repo_root).map_err(|e| GitError::CommandFailed {
            command: "git rev-parse".to_string(),
            message: e.message().to_string(),
        })?;
        let head = match repo.head() {
            Ok(head) => head,
            // Unborn branch (no commits yet) reads as detached for our purposes.
            Err(_) => return Ok(None),
        };
        if !head.is_branch() {
            return Ok(None);
        }
        Ok(head.shorthand().ok().map(str::to_string))
    }

    fn fetch(&self, repo_root: &Path, remote: &str) -> Result<(), GitError> {
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["fetch", remote])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "git fetch".to_string(),
                message: e.to_string(),
            })?;

        if output.status.success() {
            return Ok(());
        }

        Err(GitError::CommandFailed {
            command: "git fetch".to_string(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }

    fn is_branch_merged(
        &self,
        repo_root: &Path,
        branch: &crate::domain::value::BranchValue,
        into: &str,
    ) -> Result<MergeCheck, GitError> {
        let branch_str = branch.as_ref();
        let merged = || -> Result<bool, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let branch_oid = repo.revparse_single(branch_str)?.id();
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
            Ok(false) => Ok(match is_pr_merged_on_github(repo_root, branch_str) {
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

    fn delete_branch(&self, repo_root: &Path, branch: &crate::domain::value::BranchValue) -> Result<(), GitError> {
        let branch_str = branch.as_ref();
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "-d", branch_str])
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

    fn branch_exists(&self, repo_root: &Path, branch: &crate::domain::value::BranchValue) -> Result<bool, GitError> {
        let repo = git2::Repository::open(repo_root).map_err(|e| GitError::CommandFailed {
            command: "git2::Repository::open".to_string(),
            message: e.message().to_string(),
        })?;
        let found = repo.find_branch(branch.as_ref(), git2::BranchType::Local).is_ok();
        Ok(found)
    }

    fn worktree_exists(&self, repo_root: &Path, slug: &crate::domain::value::SlugValue) -> Result<bool, GitError> {
        let repo = git2::Repository::open(repo_root).map_err(|e| GitError::CommandFailed {
            command: "git2::Repository::open".to_string(),
            message: e.message().to_string(),
        })?;
        let worktrees = repo.worktrees().map_err(|e| GitError::CommandFailed {
            command: "git worktree list".to_string(),
            message: e.message().to_string(),
        })?;
        // iter() yields Result<Option<&str>, _>; flatten twice to reach &str.
        Ok(worktrees.iter().flatten().flatten().any(|name| name == slug.as_ref()))
    }

    fn add_worktree(
        &self,
        repo_root: &Path,
        path: &Path,
        branch: &crate::domain::value::BranchValue,
        start_point: &crate::domain::value::RefValue,
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
                branch.as_ref(),
                start_point.as_ref(),
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

    fn resolve_ref(&self, repo_root: &Path, ref_spec: &crate::domain::value::RefValue) -> Result<(), GitError> {
        let ref_spec_str = ref_spec.as_ref();
        let resolve = || -> Result<(), git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            repo.revparse_single(ref_spec_str)?;
            Ok(())
        };
        resolve().map_err(|e| GitError::RefResolve {
            ref_spec: ref_spec_str.to_string(),
            message: e.message().to_string(),
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
        head_ref: &crate::domain::value::RefValue,
    ) -> Result<Vec<PathBuf>, GitError> {
        let head_ref_str = head_ref.as_ref();
        let diff_paths = || -> Result<Vec<PathBuf>, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let base_oid = repo
                .revparse_single(&format!("origin/{}", base_branch))?
                .id();
            let head_oid = repo.revparse_single(head_ref_str)?.id();
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
        rev: &crate::domain::value::RefValue,
        path: &Path,
    ) -> Result<Option<String>, GitError> {
        let rev_str = rev.as_ref();
        let read = || -> Result<Option<String>, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let commit_oid = repo.revparse_single(rev_str)?.id();
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
        ancestor_ref: &crate::domain::value::RefValue,
        descendant_ref: &crate::domain::value::RefValue,
    ) -> Result<bool, GitError> {
        let ancestor_ref_str = ancestor_ref.as_ref();
        let descendant_ref_str = descendant_ref.as_ref();
        let check = || -> Result<bool, git2::Error> {
            let repo = git2::Repository::open(repo_root)?;
            let ancestor_oid = repo.revparse_single(ancestor_ref_str)?.id();
            let descendant_oid = repo.revparse_single(descendant_ref_str)?.id();

            if ancestor_oid == descendant_oid {
                return Ok(true);
            }
            repo.graph_descendant_of(descendant_oid, ancestor_oid)
        };
        check().map_err(|e| GitError::RefResolve {
            ref_spec: ancestor_ref_str.to_string(),
            message: e.message().to_string(),
        })
    }

    fn pr_state(&self, repo_root: &Path, branch: &crate::domain::value::RefValue) -> Result<PrState, GitError> {
        let branch_str = branch.as_ref();
        let output = std::process::Command::new("gh")
            .current_dir(repo_root)
            .args([
                "pr",
                "list",
                "--head",
                branch_str,
                "--state",
                "all",
                "--json",
                "state,mergedAt",
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

        // A branch can carry several historical PRs (a closed attempt, then a
        // reopened one). Rank across all of them rather than trusting gh's
        // default ordering: an open PR wins, else a merged one, else the
        // branch is abandoned.
        let classify = |pr: &serde_json::Value| -> PrState {
            let merged_at = pr.get("mergedAt");
            if merged_at.is_some() && merged_at != Some(&serde_json::Value::Null) {
                return PrState::Merged;
            }
            match pr.get("state").and_then(|v| v.as_str()) {
                Some("OPEN") => PrState::Open,
                _ => PrState::ClosedUnmerged,
            }
        };

        let rank = |state: &PrState| match state {
            PrState::Open => 3,
            PrState::Merged => 2,
            PrState::ClosedUnmerged => 1,
            PrState::None => 0,
        };

        Ok(arr
            .iter()
            .map(classify)
            .max_by_key(|state| rank(state))
            .unwrap_or(PrState::None))
    }

    fn rebase(&self, repo_root: &Path, onto: &crate::domain::value::RefValue) -> Result<(), GitError> {
        if rebase_in_progress(repo_root) {
            return continue_rebase(repo_root);
        }

        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["rebase", onto.as_ref()])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "git rebase".to_string(),
                message: e.to_string(),
            })?;

        if output.status.success() {
            return Ok(());
        }

        Err(GitError::Rebase {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }

    fn merge(&self, repo_root: &Path, other_ref: &crate::domain::value::RefValue) -> Result<(), GitError> {
        if merge_in_progress(repo_root) {
            return continue_merge(repo_root);
        }

        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["merge", "--no-edit", other_ref.as_ref()])
            .output()
            .map_err(|e| GitError::CommandFailed {
                command: "git merge".to_string(),
                message: e.to_string(),
            })?;

        if output.status.success() {
            return Ok(());
        }

        Err(GitError::Merge {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Resolves a git-dir-relative path (e.g. `rebase-merge`, `MERGE_HEAD`) via
/// git's own porcelain (`git rev-parse --git-path`), which correctly
/// accounts for worktrees where `.git` is a file pointing elsewhere rather
/// than a directory.
fn git_path(repo_root: &Path, relative: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "--git-path", relative])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw);
    Some(if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    })
}

fn rebase_in_progress(repo_root: &Path) -> bool {
    git_path(repo_root, "rebase-merge").is_some_and(|p| p.exists())
        || git_path(repo_root, "rebase-apply").is_some_and(|p| p.exists())
}

fn merge_in_progress(repo_root: &Path) -> bool {
    git_path(repo_root, "MERGE_HEAD").is_some_and(|p| p.exists())
}

fn continue_rebase(repo_root: &Path) -> Result<(), GitError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["rebase", "--continue"])
        .output()
        .map_err(|e| GitError::CommandFailed {
            command: "git rebase --continue".to_string(),
            message: e.to_string(),
        })?;

    if output.status.success() {
        return Ok(());
    }

    Err(GitError::Rebase {
        message: format!(
            "rebase still has unresolved conflicts: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    })
}

fn continue_merge(repo_root: &Path) -> Result<(), GitError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["commit", "--no-edit"])
        .output()
        .map_err(|e| GitError::CommandFailed {
            command: "git commit --no-edit".to_string(),
            message: e.to_string(),
        })?;

    if output.status.success() {
        return Ok(());
    }

    Err(GitError::Merge {
        message: format!(
            "merge still has unresolved conflicts: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    })
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
