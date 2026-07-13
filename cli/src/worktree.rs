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

    mod worktree_add {
        use super::*;

        #[test]
        fn detects_main_branch_from_validation_md() {
            let temp_dir = TempDir::new().expect("failed to create temp directory");
            let repo_root = temp_dir.path();

            run_git(repo_root, &["init", "-q"]);
            run_git(repo_root, &["config", "user.email", "test@example.com"]);
            run_git(repo_root, &["config", "user.name", "Test"]);

            fs::write(
                repo_root.join("validation.md"),
                "## PR conventions\n- Main branch: main\n- Commit style: whatever\n",
            )
            .expect("failed to write validation.md");

            run_git(repo_root, &["add", "."]);
            run_git(repo_root, &["commit", "-q", "-m", "init"]);

            let branch = crate::worktree::detect_main_branch(repo_root);
            assert_eq!(branch, "main");
        }

        #[test]
        fn detects_main_branch_via_git_fallback() {
            let temp_dir = TempDir::new().expect("failed to create temp directory");
            let repo_root = temp_dir.path();

            run_git(repo_root, &["init", "-q", "-b", "main"]);
            run_git(repo_root, &["config", "user.email", "test@example.com"]);
            run_git(repo_root, &["config", "user.name", "Test"]);

            fs::write(repo_root.join("README.md"), "hello").expect("failed to write file");
            run_git(repo_root, &["add", "."]);
            run_git(repo_root, &["commit", "-q", "-m", "init"]);

            // No validation.md present in this repo.
            let branch = crate::worktree::detect_main_branch(repo_root);
            assert_eq!(branch, "main");
        }
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

        // No .gitignore exists yet
        assert!(!repo_root.join(".gitignore").exists());

        // Call ensure_worktrees_ignored
        let changed = crate::worktree::ensure_worktrees_ignored(repo_root);
        assert!(changed, "should return true when .gitignore was missing");

        // Verify .gitignore now exists and contains .worktrees/
        let gitignore_content = fs::read_to_string(repo_root.join(".gitignore"))
            .expect("failed to read .gitignore");
        assert!(gitignore_content.contains(".worktrees/"), ".gitignore should contain .worktrees/");
    }

    #[test]
    fn leaves_existing_gitignore_entry_alone() {
        let temp_dir = TempDir::new().expect("failed to create temp directory");
        let repo_root = temp_dir.path();

        run_git(repo_root, &["init", "-q"]);
        run_git(repo_root, &["config", "user.email", "test@example.com"]);
        run_git(repo_root, &["config", "user.name", "Test"]);

        // Create .gitignore with .worktrees/ already present
        fs::write(repo_root.join(".gitignore"), ".worktrees/\n").expect("failed to write .gitignore");
        fs::write(repo_root.join("README.md"), "hello").expect("failed to write file");
        run_git(repo_root, &["add", "-A"]);
        run_git(repo_root, &["commit", "-q", "-m", "init"]);

        let original_content = fs::read_to_string(repo_root.join(".gitignore"))
            .expect("failed to read .gitignore");

        // Call ensure_worktrees_ignored
        let changed = crate::worktree::ensure_worktrees_ignored(repo_root);
        assert!(!changed, "should return false when .worktrees/ is already ignored");

        // Verify .gitignore hasn't changed
        let new_content = fs::read_to_string(repo_root.join(".gitignore"))
            .expect("failed to read .gitignore");
        assert_eq!(original_content, new_content, ".gitignore should not be modified");
    }
}

/// Detect the repository's main branch.
///
/// Prefers a `Main branch: <name>` line under `## PR conventions` in
/// `validation.md` at `repo_root` if present; otherwise falls back to git
/// (origin's default branch, or the current branch if there's no remote).
pub(crate) fn detect_main_branch(repo_root: &std::path::Path) -> String {
    let validation_path = repo_root.join("validation.md");
    if let Ok(text) = std::fs::read_to_string(&validation_path) {
        if let Some(branch) = parse_main_branch_from_validation(&text) {
            return branch;
        }
    }

    detect_main_branch_via_git(repo_root)
}

fn parse_main_branch_from_validation(text: &str) -> Option<String> {
    let mut in_pr_conventions = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("##") {
            in_pr_conventions = trimmed.trim_start_matches('#').trim().eq_ignore_ascii_case("PR conventions");
            continue;
        }
        if in_pr_conventions {
            let bullet = trimmed.trim_start_matches(['-', '*']).trim();
            if let Some(rest) = bullet.strip_prefix("Main branch:") {
                let branch = rest.trim().trim_matches('`').trim();
                if !branch.is_empty() {
                    return Some(branch.to_string());
                }
            }
        }
    }
    None
}

fn detect_main_branch_via_git(repo_root: &std::path::Path) -> String {
    // Try origin's default branch first.
    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_root)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(name) = raw.rsplit('/').next() {
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }

    // Fall back to the current branch (no remote configured).
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()
        .expect("failed to run git branch --show-current");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Ensure `.worktrees/` is ignored in the repository's `.gitignore`.
///
/// Checks if `.worktrees/` is already ignored in the `.gitignore` file.
/// If not, appends it to `.gitignore` (creating the file if absent) and returns `true`.
/// Returns `false` if `.worktrees/` was already ignored (no changes made).
pub(crate) fn ensure_worktrees_ignored(repo_root: &std::path::Path) -> bool {
    let gitignore_path = repo_root.join(".gitignore");

    // Check if already in .gitignore file
    if gitignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
            if content.contains(".worktrees/") {
                return false; // Already in file, no change needed
            }
        }
    }

    // .worktrees/ is not in .gitignore, add it
    let mut content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Ensure content ends with newline if it's not empty
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }

    content.push_str(".worktrees/\n");

    std::fs::write(&gitignore_path, &content)
        .expect("failed to write .gitignore");

    true // Changed
}

/// Check if a worktree for the given slug already exists.
///
/// Queries `git worktree list` to see if `.worktrees/<slug>` is registered.
/// Returns `true` if the worktree exists, `false` otherwise.
pub(crate) fn worktree_exists(repo_root: &std::path::Path, slug: &str) -> bool {
    let worktree_path = format!(".worktrees/{}", slug);

    let output = std::process::Command::new("git")
        .args(["worktree", "list"])
        .current_dir(repo_root)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let list_str = String::from_utf8_lossy(&output.stdout);
            return list_str.contains(&worktree_path);
        }
    }

    false
}
