use crate::domain::error::StateError;

/// Owns `.heist/<slug>/`'s lifecycle as a directory: creation and wholesale
/// removal. Distinct from `StateRepository`, which owns `state.json`'s
/// content inside that directory.
pub trait HeistDirRepository {
    /// Creates `.heist/<slug>/`. Errors with `AlreadyExists` if it's already
    /// there (a pre-existing but empty dir still counts as a collision).
    fn create(&self, slug: &str) -> Result<(), StateError>;

    /// Removes `.heist/<slug>/` entirely. A no-op (Ok) if it doesn't exist.
    fn remove(&self, slug: &str) -> Result<(), StateError>;
}
