use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::domain::value::SlugValue;
use crate::ports::state_repository::StateRepository;

pub fn resume(repo: &dyn StateRepository, slug: &SlugValue) -> Result<State, StateError> {
    repo.load(slug)
}
