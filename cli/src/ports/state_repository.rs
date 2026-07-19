use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::SlugValue;

pub trait StateRepository {
    fn exists(&self, slug: &str) -> bool;

    fn init(&self, slug: &str, state: &State) -> Result<(), StateError>;

    fn load(&self, slug: &str) -> Result<State, StateError>;

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError>;

    fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError>;

    /// Reads `.heist/<slug>/score.md`. `Ok(None)` means the file doesn't
    /// exist (score.md is optional until the Forger writes it); `Err` is a
    /// true IO failure (permissions, corrupt filesystem, etc).
    fn load_score(&self, slug: &str) -> Result<Option<String>, std::io::Error>;
}
