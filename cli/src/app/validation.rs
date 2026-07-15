use crate::domain::validation::{self, ValidationError};
use crate::ports::validation_source::ValidationSource;
use std::path::{Path, PathBuf};

pub fn resolve(src: &dyn ValidationSource, paths: &[PathBuf]) -> Result<String, ValidationError> {
    if paths.len() == 1 {
        validation::resolve_validation(src, &paths[0])
    } else {
        validation::resolve_validations(src, paths)
    }
}

pub fn check(src: &dyn ValidationSource, path: &Path) -> Result<bool, ValidationError> {
    validation::check_validation_exists(src, path)
}
