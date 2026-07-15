use std::error::Error;
use std::path::{Path, PathBuf};

pub trait ValidationSource {
    fn repo_root(&self) -> Result<PathBuf, Box<dyn Error>>;

    fn read_validation(&self, dir: &Path) -> Result<Option<String>, Box<dyn Error>>;
}
