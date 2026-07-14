use crate::domain::error::StateError;
use crate::domain::state::State;
use crate::ports::state_repository::StateRepository;

pub fn resume(repo: &dyn StateRepository, slug: &str) -> Result<State, StateError> {
    repo.load(slug)
}
