use crate::domain::error::StateError;
use crate::domain::state::{Stage, State};
use crate::domain::value::{NonBlankValue, SlugValue};
use crate::ports::state_repository::StateRepository;

pub struct ListRow {
    pub slug: SlugValue,
    pub stage: Stage,
    pub next_step: Option<Stage>,
    pub worktree: Option<NonBlankValue>,
}

impl From<&State> for ListRow {
    fn from(state: &State) -> Self {
        ListRow {
            slug: state.slug.clone(),
            stage: state.stage,
            next_step: state.stage.next_step(),
            worktree: state.worktree.clone(),
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
