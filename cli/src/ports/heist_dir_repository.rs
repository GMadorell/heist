use crate::domain::error::StateError;
use crate::domain::value::SlugValue;

/// Owns `.heist/<slug>/`'s lifecycle as a directory
pub trait HeistDirRepository {
    fn create(&self, slug: &SlugValue) -> Result<(), StateError>;

    fn remove(&self, slug: &SlugValue) -> Result<(), StateError>;
}
