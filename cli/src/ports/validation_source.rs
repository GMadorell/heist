use std::error::Error;
use std::path::{Path, PathBuf};

pub trait ValidationSource {
    /// Walk up from cwd to the repo root (the `.git` marker today).
    fn repo_root(&self) -> Result<PathBuf, Box<dyn Error>>;

    /// Read `validation.md` in `dir`, or None if it is absent.
    fn read_validation(&self, dir: &Path) -> Result<Option<String>, Box<dyn Error>>;
}
