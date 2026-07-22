use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::SlugValue;

pub trait StateRepository {
    fn exists(&self, slug: &SlugValue) -> bool;

    fn load(&self, slug: &SlugValue) -> Result<State, StateError>;

    fn save(&self, slug: &SlugValue, state: &State) -> Result<(), StateError>;

    fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError>;
}
