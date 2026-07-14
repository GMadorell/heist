use crate::domain::error::FieldError;
use crate::domain::value::NonBlankValue;
use std::path::{Path, PathBuf};

pub fn worktree_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join(".worktrees").join(slug)
}

pub fn branch_name(slug: &str) -> Result<NonBlankValue, FieldError> {
    NonBlankValue::parse("branch", &format!("heist/{}", slug))
}
