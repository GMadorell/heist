use crate::domain::error::ValueError;
use crate::domain::value::{BranchValue, SlugValue};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeistWorktree {
    pub slug: SlugValue,
    pub path: PathBuf,
    pub branch: BranchValue,
}

impl HeistWorktree {
    /// Lifts a raw (path, branch) pair reported by `list_worktrees` into a
    /// validated heist-owned worktree, or `None` if it isn't one.
    /// `canonical_repo_root` must already be canonicalized by the caller.
    pub fn try_from_parts(
        path: &Path,
        branch: Option<&str>,
        canonical_repo_root: &Path,
    ) -> Option<HeistWorktree> {
        let worktrees_dir = canonical_repo_root.join(".worktrees");
        if path.parent()? != worktrees_dir {
            return None;
        }
        let basename = path.file_name()?.to_str()?;
        let slug = SlugValue::parse(basename).ok()?;
        let expected_branch = branch_name(slug.as_ref()).ok()?;
        if branch != Some(expected_branch.as_ref()) {
            return None;
        }
        Some(HeistWorktree {
            slug,
            path: path.to_path_buf(),
            branch: expected_branch,
        })
    }
}

pub fn worktree_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join(".worktrees").join(slug)
}

pub fn branch_name(slug: &str) -> Result<BranchValue, ValueError> {
    BranchValue::try_from_raw("branch", &format!("heist/{}", slug))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn lifts_heist_owned_worktree() {
        let repo_root = Path::new("/repo");
        let path = Path::new("/repo/.worktrees/foo");

        let result = HeistWorktree::try_from_parts(path, Some("heist/foo"), repo_root);

        let hw = result.expect("should lift a heist-owned worktree");
        assert_eq!(hw.slug.as_ref(), "foo");
        assert_eq!(hw.path, path);
        assert_eq!(hw.branch.as_ref(), "heist/foo");
    }

    #[test]
    fn rejects_mismatched_branch() {
        let repo_root = Path::new("/repo");
        let path = Path::new("/repo/.worktrees/foo");

        let result = HeistWorktree::try_from_parts(path, Some("some-other-branch"), repo_root);

        assert!(result.is_none());
    }

    #[test]
    fn rejects_path_outside_worktrees_dir() {
        let repo_root = Path::new("/repo");
        let path = Path::new("/repo/elsewhere/foo");

        let result = HeistWorktree::try_from_parts(path, Some("heist/foo"), repo_root);

        assert!(result.is_none());
    }

    #[test]
    fn rejects_detached_head() {
        let repo_root = Path::new("/repo");
        let path = Path::new("/repo/.worktrees/foo");

        let result = HeistWorktree::try_from_parts(path, None, repo_root);

        assert!(result.is_none());
    }
}
