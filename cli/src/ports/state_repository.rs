use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::SlugValue;

pub trait StateRepository {
    fn exists(&self, slug: &str) -> bool;

    fn init(&self, slug: &str, state: &State) -> Result<(), StateError>;

    fn load(&self, slug: &str) -> Result<State, StateError>;

    fn save(&self, slug: &str, state: &State) -> Result<(), StateError>;

    fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError>;

    /// Removes `.heist/<slug>/` entirely. A no-op (Ok) if it doesn't exist.
    fn remove(&self, slug: &str) -> Result<(), StateError>;
}
