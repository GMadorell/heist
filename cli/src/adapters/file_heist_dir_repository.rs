use crate::domain::error::StateError;
use crate::domain::value::SlugValue;
use crate::ports::heist_dir_repository::HeistDirRepository;
use std::path::{Path, PathBuf};

pub struct FileHeistDirRepository;

impl HeistDirRepository for FileHeistDirRepository {
    fn create(&self, slug: &SlugValue) -> Result<(), StateError> {
        let dir = heist_dir_path(slug);
        // Reject on directory existence (not file existence) so a pre-existing
        // but empty `.heist/<slug>/` still counts as "already initialised".
        if dir.exists() {
            return Err(StateError::AlreadyExists);
        }
        std::fs::create_dir_all(&dir).map_err(StateError::Unreadable)
    }

    fn remove(&self, slug: &SlugValue) -> Result<(), StateError> {
        let dir = heist_dir_path(slug);
        if !dir.exists() {
            return Ok(());
        }
        std::fs::remove_dir_all(&dir).map_err(StateError::Unreadable)
    }
}

pub(crate) fn heist_dir_path(slug: &SlugValue) -> PathBuf {
    Path::new(".heist").join(slug.as_ref())
}
