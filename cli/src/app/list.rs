use crate::domain::error::StateError;
use crate::domain::state::{Mode, Routing, Stage, State, route};
use crate::domain::value::{NonBlankValue, SlugValue};
use crate::ports::state_repository::StateRepository;

pub struct ListRow {
    pub slug: SlugValue,
    pub stage: Stage,
    pub next: Option<Routing>,
    pub worktree: Option<NonBlankValue>,
    pub mode: Mode,
}

impl From<&State> for ListRow {
    fn from(state: &State) -> Self {
        ListRow {
            slug: state.slug.clone(),
            stage: state.stage,
            next: route(state.stage, state.mode),
            worktree: state.worktree.clone(),
            mode: state.mode,
        }
    }
}

pub enum ListError {
    ListSlugs(StateError),
    Load { slug: SlugValue, error: StateError },
}

pub fn list(repo: &dyn StateRepository) -> Result<Vec<ListRow>, ListError> {
    let slugs = repo.list_slugs().map_err(ListError::ListSlugs)?;

    let mut rows = Vec::with_capacity(slugs.len());
    for slug in slugs {
        let state = repo.load(slug.as_ref()).map_err(|error| ListError::Load {
            slug: slug.clone(),
            error,
        })?;
        rows.push(ListRow::from(&state));
    }
    Ok(rows)
}
