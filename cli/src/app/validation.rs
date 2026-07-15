use crate::domain::validation;
use crate::ports::validation_source::ValidationSource;
use std::error::Error;
use std::path::{Path, PathBuf};

pub fn resolve(src: &dyn ValidationSource, paths: &[PathBuf]) -> Result<String, Box<dyn Error>> {
    if paths.len() == 1 {
        validation::resolve_validation(src, &paths[0])
    } else {
        validation::resolve_validations(src, paths)
    }
}

pub fn check(src: &dyn ValidationSource, path: &Path) -> Result<bool, Box<dyn Error>> {
    validation::check_validation_exists(src, path)
}
