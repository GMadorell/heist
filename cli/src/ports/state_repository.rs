use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::SlugValue;

pub trait StateRepository {
    fn exists(&self, slug: &crate::domain::value::SlugValue) -> bool;

    fn load(&self, slug: &crate::domain::value::SlugValue) -> Result<State, StateError>;

    fn save(&self, slug: &crate::domain::value::SlugValue, state: &State) -> Result<(), StateError>;

    fn list_slugs(&self) -> Result<Vec<SlugValue>, StateError>;
}
