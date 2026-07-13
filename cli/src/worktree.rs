use std::path::Path;

/// Detect the repository's main branch, always via git.
///
/// Prefers origin's default branch (`refs/remotes/origin/HEAD`); falls back to
/// the current branch when there's no remote. The `Main branch:` line in
/// `validation.md` is for human-facing agents (Cleaner, etc.), not for the
/// deterministic CLI, which shouldn't parse prose for something git knows.
pub(crate) fn detect_main_branch(repo_root: &Path) -> String {
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

/// Whether `branch` is merged into `origin/<main_branch>`.
///
/// True when the branch tip equals the main tip (e.g. after a fast-forward
/// merge) or is an ancestor of it.
pub(crate) fn branch_merged_into_main(
    repo_root: &Path,
    branch: &str,
    main_branch: &str,
) -> Result<bool, git2::Error> {
    let repo = git2::Repository::open(repo_root)?;
    let branch_oid = repo.revparse_single(branch)?.id();
    let main_oid = repo
        .revparse_single(&format!("origin/{}", main_branch))?
        .id();

    if branch_oid == main_oid {
        return Ok(true);
    }
    repo.graph_descendant_of(main_oid, branch_oid)
}

/// Ensure `.worktrees/` is ignored in the repository's `.gitignore`.
///
/// Appends the entry (creating the file if absent) and returns `true`, or
/// returns `false` when it was already ignored (no changes made).
pub(crate) fn ensure_worktrees_ignored(repo_root: &Path) -> bool {
    let gitignore_path = repo_root.join(".gitignore");

    if gitignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
            if content.contains(".worktrees/") {
                return false;
            }
        }
    }

    let mut content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(".worktrees/\n");

    std::fs::write(&gitignore_path, &content).expect("failed to write .gitignore");
    true
}

/// Whether a worktree for `slug` is already registered (`.worktrees/<slug>`).
pub(crate) fn worktree_exists(repo_root: &Path, slug: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn run_git(dir: &std::path::Path, args: &[&str]) {
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

        let branch = crate::worktree::detect_main_branch(repo_root);
        assert_eq!(branch, "main");
    }

    #[test]
    fn adds_worktrees_to_missing_gitignore() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo_root = temp_dir.path();

        run_git(repo_root, &["init", "-q"]);
        run_git(repo_root, &["config", "user.email", "test@example.com"]);
        run_git(repo_root, &["config", "user.name", "Test"]);

        fs::write(repo_root.join("README.md"), "hello").expect("failed to write file");
        run_git(repo_root, &["add", "."]);
        run_git(repo_root, &["commit", "-q", "-m", "init"]);

        assert!(!repo_root.join(".gitignore").exists());

        let changed = crate::worktree::ensure_worktrees_ignored(repo_root);
        assert!(changed, "should return true when .gitignore was missing");

        let gitignore_content =
            fs::read_to_string(repo_root.join(".gitignore")).expect("failed to read .gitignore");
        assert!(
            gitignore_content.contains(".worktrees/"),
            ".gitignore should contain .worktrees/"
        );
    }

    #[test]
    fn leaves_existing_gitignore_entry_alone() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo_root = temp_dir.path();

        run_git(repo_root, &["init", "-q"]);
        run_git(repo_root, &["config", "user.email", "test@example.com"]);
        run_git(repo_root, &["config", "user.name", "Test"]);

        fs::write(repo_root.join(".gitignore"), ".worktrees/\n")
            .expect("failed to write .gitignore");
        fs::write(repo_root.join("README.md"), "hello").expect("failed to write file");
        run_git(repo_root, &["add", "-A"]);
        run_git(repo_root, &["commit", "-q", "-m", "init"]);

        let original_content =
            fs::read_to_string(repo_root.join(".gitignore")).expect("failed to read .gitignore");

        let changed = crate::worktree::ensure_worktrees_ignored(repo_root);
        assert!(
            !changed,
            "should return false when .worktrees/ is already ignored"
        );

        let new_content =
            fs::read_to_string(repo_root.join(".gitignore")).expect("failed to read .gitignore");
        assert_eq!(
            original_content, new_content,
            ".gitignore should not be modified"
        );
    }
}
