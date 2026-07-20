use crate::domain::error::StateError;

/// Owns `.heist/<slug>/`'s lifecycle as a directory
pub trait HeistDirRepository {
    fn create(&self, slug: &str) -> Result<(), StateError>;

    fn remove(&self, slug: &str) -> Result<(), StateError>;
}
