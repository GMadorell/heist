use std::error::Error;
use std::path::{Path, PathBuf};

pub trait ValidationSource {
    fn repo_root(&self) -> Result<PathBuf, Box<dyn Error>>;

    fn read_validation(&self, dir: &Path) -> Result<Option<String>, Box<dyn Error>>;

    fn exists(&self, path: &Path) -> bool;

    fn is_dir(&self, path: &Path) -> bool;

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, Box<dyn Error>>;
}
