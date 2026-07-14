use crate::ports::worktree_fs::WorktreeFs;
use std::fs;
use std::path::{Path, PathBuf};

pub struct FilesystemWorktree;

impl WorktreeFs for FilesystemWorktree {
    fn ensure_worktrees_ignored(&self, repo_root: &Path) -> std::io::Result<bool> {
        let gitignore_path = repo_root.join(".gitignore");

        if gitignore_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
                if content.contains(".worktrees/") {
                    return Ok(false);
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

        std::fs::write(&gitignore_path, &content)?;
        Ok(true)
    }

    fn link_heist_dir(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
        slug: &str,
    ) -> std::io::Result<()> {
        let main_heist_canonical = repo_root.join(".heist").join(slug).canonicalize()?;

        let worktree_heist_dir = worktree_path.join(".heist");
        if !worktree_heist_dir.exists() {
            fs::create_dir_all(&worktree_heist_dir)?;
        }

        let worktree_heist_slug = worktree_heist_dir.join(slug);
        if worktree_heist_slug.exists() {
            fs::remove_file(&worktree_heist_slug)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs as unix_fs;
            unix_fs::symlink(&main_heist_canonical, &worktree_heist_slug)?;
        }

        #[cfg(not(unix))]
        {
            let _ = main_heist_canonical;
            return Err(std::io::Error::other(
                "symlink creation not supported on this platform",
            ));
        }

        Ok(())
    }

    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
        path.canonicalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let changed = FilesystemWorktree
            .ensure_worktrees_ignored(repo_root)
            .expect("should succeed");
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

        let changed = FilesystemWorktree
            .ensure_worktrees_ignored(repo_root)
            .expect("should succeed");
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
