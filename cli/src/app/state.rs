use crate::domain::error::{FieldError, StateError};
use crate::domain::state::State;
use crate::domain::value::DateValue;
use crate::ports::clock::Clock;
use crate::ports::state_repository::StateRepository;

pub enum InitError {
    InvalidSlug(FieldError),
    Init(StateError),
}

pub fn init(repo: &dyn StateRepository, clock: &dyn Clock, slug: &str) -> Result<(), InitError> {
    let state = State::new(slug, clock.today()).map_err(InitError::InvalidSlug)?;
    repo.init(slug, &state).map_err(InitError::Init)
}

pub enum GetError {
    Load(StateError),
    Field(FieldError),
}

pub fn get(repo: &dyn StateRepository, slug: &str, field: &str) -> Result<String, GetError> {
    let state = repo.load(slug).map_err(GetError::Load)?;
    state.get_field(field).map_err(GetError::Field)
}

pub enum SetError {
    Field(FieldError),
    Load(StateError),
    Save(StateError),
}

pub fn set(
    repo: &dyn StateRepository,
    clock: &dyn Clock,
    slug: &str,
    field: &str,
    value: &str,
) -> Result<(), SetError> {
    let mut state = repo.load(slug).map_err(SetError::Load)?;
    state.set_field(field, value).map_err(SetError::Field)?;
    state.updated = clock.today();
    repo.save(slug, &state).map_err(SetError::Save)
}

pub enum SchemaError {
    InvalidExample(FieldError),
    Serialize(serde_json::Error),
}

const FIELD_LIST: &str = "schema_version: u32\n\
slug: string\n\
stage: string (casing|planning|fence_review|human_review|forging|safehouse|implementing|cleaning|done)\n\
worktree: string|null\n\
branch: string|null\n\
score_step: u32\n\
score_steps_total: u32\n\
fence_rounds: u32\n\
created: string\n\
updated: string";

pub fn schema() -> Result<String, SchemaError> {
    // The schema output shows example strings, so a constant date keeps it
    // deterministic rather than reflecting the day the command runs.
    let example_date = DateValue::parse("created", "2026-01-01").expect("constant date is valid");
    let example = State::new("example", example_date).map_err(SchemaError::InvalidExample)?;
    let json = serde_json::to_string_pretty(&example).map_err(SchemaError::Serialize)?;
    Ok(format!("{}\n\n{}", FIELD_LIST, json))
}
