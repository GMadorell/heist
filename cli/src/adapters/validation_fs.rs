use crate::ports::validation_source::ValidationSource;
use std::error::Error;
use std::path::{Path, PathBuf};

pub struct ValidationFs;

impl ValidationSource for ValidationFs {
    fn repo_root(&self) -> Result<PathBuf, Box<dyn Error>> {
        let repo = git2::Repository::discover(".")?;
        let workdir = repo
            .workdir()
            .ok_or("repository has no working directory")?;
        Ok(workdir.to_path_buf())
    }

    fn cwd(&self) -> Result<PathBuf, Box<dyn Error>> {
        Ok(std::env::current_dir()?)
    }

    fn read_validation(&self, dir: &Path) -> Result<Option<String>, Box<dyn Error>> {
        let file = dir.join("validation.md");
        if !file.exists() {
            return Ok(None);
        }
        Ok(Some(std::fs::read_to_string(&file)?))
    }
}
