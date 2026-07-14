use std::path::{Path, PathBuf};

pub trait WorktreeFs {
    /// Ensure `.worktrees/` is in .gitignore. Returns true if it added it.
    fn ensure_worktrees_ignored(&self, repo_root: &Path) -> std::io::Result<bool>;

    /// Symlink the worktree's `.heist/<slug>` back to the main `.heist/<slug>`.
    fn link_heist_dir(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
        slug: &str,
    ) -> std::io::Result<()>;

    /// Canonicalize a path (resolve symlinks, make absolute).
    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf>;
}
