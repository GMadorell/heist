#[cfg(test)]
mod tests {
    mod worktree_add {
        use std::fs;
        use std::process::Command;
        use tempfile::TempDir;

        fn run_git(dir: &std::path::Path, args: &[&str]) {
            let status = Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .expect("failed to run git");
            assert!(status.success(), "git {:?} failed", args);
        }

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
