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
mode: string (heavy|medium|light)\n\
worktree: string|null\n\
branch: string|null\n\
base: string|null\n\
score_wave: u32\n\
score_waves_total: u32\n\
score_steps_total: u32\n\
fence_rounds: u32\n\
created: string\n\
updated: string";

pub fn schema() -> Result<String, SchemaError> {
    let example_date = DateValue::parse("created", "2026-01-01").expect("constant date is valid");
    let example = State::new("example", example_date).map_err(SchemaError::InvalidExample)?;
    let json = serde_json::to_string_pretty(&example).map_err(SchemaError::Serialize)?;
    Ok(format!("{}\n\n{}", FIELD_LIST, json))
}

#[derive(Debug)]
pub enum IncrError {
    Load(StateError),
    Field(FieldError),
    Save(StateError),
}

pub fn incr(
    repo: &dyn StateRepository,
    clock: &dyn Clock,
    slug: &str,
    field: &str,
) -> Result<(), IncrError> {
    let mut state = repo.load(slug).map_err(IncrError::Load)?;
    let current = state.get_field(field).map_err(IncrError::Field)?;
    let parsed: u32 = current
        .parse()
        .map_err(|_| IncrError::Field(FieldError::NotIncrementable(field.to_string())))?;
    let incremented = parsed.checked_add(1).ok_or_else(|| {
        IncrError::Field(FieldError::InvalidNumeric {
            field: field.to_string(),
            value: current.clone(),
        })
    })?;
    state
        .set_field(field, &incremented.to_string())
        .map_err(IncrError::Field)?;
    state.updated = clock.today();
    repo.save(slug, &state).map_err(IncrError::Save)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testing::{FixedClock, InMemoryStateRepository};
    use crate::domain::state::State;
    use crate::domain::value::ScoreWave;

    fn created_date() -> DateValue {
        DateValue::parse("today", "2026-01-01").expect("valid date")
    }

    fn today_date() -> DateValue {
        DateValue::parse("today", "2026-01-02").expect("valid date")
    }

    #[test]
    fn incr_increments_numeric_field_and_bumps_updated() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new("foo", created_date()).expect("valid slug"),
        );
        let clock = FixedClock(today_date());

        incr(&repo, &clock, "foo", "score_wave").expect("incr should succeed");

        let state = repo.get("foo").expect("state should exist");
        assert_eq!(state.score_wave, ScoreWave::new(1));
        assert_eq!(state.updated, today_date());
    }

    #[test]
    fn incr_rejects_non_numeric_field() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new("foo", created_date()).expect("valid slug"),
        );
        let clock = FixedClock(today_date());

        let err = incr(&repo, &clock, "foo", "stage").expect_err("should reject non-numeric field");
        match err {
            IncrError::Field(FieldError::NotIncrementable(field)) => assert_eq!(field, "stage"),
            _ => panic!("expected IncrError::Field(NotIncrementable), got a different variant"),
        }
    }

    #[test]
    fn incr_rejects_unknown_field() {
        let repo = InMemoryStateRepository::new().with_state(
            "foo",
            State::new("foo", created_date()).expect("valid slug"),
        );
        let clock = FixedClock(today_date());

        let err =
            incr(&repo, &clock, "foo", "bogus_field").expect_err("should reject unknown field");
        match err {
            IncrError::Field(FieldError::Unknown(field)) => assert_eq!(field, "bogus_field"),
            _ => panic!("expected IncrError::Field(Unknown), got a different variant"),
        }
    }

    #[test]
    fn incr_rejects_overflow_at_u32_max() {
        let mut state = State::new("foo", created_date()).expect("valid slug");
        state
            .set_field("score_wave", &u32::MAX.to_string())
            .expect("u32::MAX is a valid score_wave value");
        let repo = InMemoryStateRepository::new().with_state("foo", state);
        let clock = FixedClock(today_date());

        let err = incr(&repo, &clock, "foo", "score_wave")
            .expect_err("should reject increment past u32::MAX");
        match err {
            IncrError::Field(FieldError::InvalidNumeric { field, value }) => {
                assert_eq!(field, "score_wave");
                assert_eq!(value, u32::MAX.to_string());
            }
            _ => panic!("expected IncrError::Field(InvalidNumeric), got a different variant"),
        }

        let state = repo.get("foo").expect("state should exist");
        assert_eq!(
            state.score_wave,
            ScoreWave::new(u32::MAX),
            "state must be unchanged on overflow rejection"
        );
    }
}
