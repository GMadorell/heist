use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub slug: String,
    pub stage: Stage,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub score_step: u32,
    pub score_steps_total: u32,
    pub fence_rounds: u32,
    pub created: String,
    pub updated: String,
}

impl State {
    pub fn new(slug: &str) -> Self {
        let today = today();
        State {
            schema_version: CURRENT_SCHEMA_VERSION,
            slug: slug.to_string(),
            stage: Stage::Casing,
            worktree: None,
            branch: None,
            score_step: 0,
            score_steps_total: 0,
            fence_rounds: 0,
            created: today.clone(),
            updated: today,
        }
    }

    /// Read, parse, and validate a state file into the typed `State`.
    ///
    /// Centralizes the read/deserialize/schema-check the CLI used to repeat at
    /// every call site, so nobody hand-parses `state.json` as a raw `Value`.
    pub fn load(path: &Path) -> Result<State, StateError> {
        if !path.exists() {
            return Err(StateError::Missing);
        }
        let content = std::fs::read_to_string(path).map_err(StateError::Unreadable)?;
        let state: State = serde_json::from_str(&content).map_err(StateError::Unparseable)?;
        if state.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(StateError::SchemaMismatch {
                found: state.schema_version,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(state)
    }

    /// Serialize and write the state to `path` (pretty-printed).
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        let json = serde_json::to_string_pretty(self).map_err(StateError::Unparseable)?;
        std::fs::write(path, json).map_err(StateError::Unreadable)
    }

    /// Render a single field as the string the CLI prints for `state get`.
    pub fn get_field(&self, field: &str) -> Result<String, FieldError> {
        let value = match field {
            "schema_version" => self.schema_version.to_string(),
            "slug" => self.slug.clone(),
            "stage" => self.stage.as_str().to_string(),
            "worktree" => self.worktree.clone().unwrap_or_else(|| "null".to_string()),
            "branch" => self.branch.clone().unwrap_or_else(|| "null".to_string()),
            "score_step" => self.score_step.to_string(),
            "score_steps_total" => self.score_steps_total.to_string(),
            "fence_rounds" => self.fence_rounds.to_string(),
            "created" => self.created.clone(),
            "updated" => self.updated.clone(),
            _ => return Err(FieldError::Unknown(field.to_string())),
        };
        Ok(value)
    }

    /// Assign a known field by name, parsing into its typed representation.
    ///
    /// Keeps field-name knowledge next to the struct instead of in CLI plumbing,
    /// and never touches a raw `serde_json::Value`.
    pub fn set_field(&mut self, field: &str, value: &str) -> Result<(), FieldError> {
        match field {
            "schema_version" => self.schema_version = parse_numeric(field, value)?,
            "slug" => self.slug = value.to_string(),
            "stage" => self.stage = Stage::parse(value)?,
            "worktree" => self.worktree = Some(value.to_string()),
            "branch" => self.branch = Some(value.to_string()),
            "score_step" => self.score_step = parse_numeric(field, value)?,
            "score_steps_total" => self.score_steps_total = parse_numeric(field, value)?,
            "fence_rounds" => self.fence_rounds = parse_numeric(field, value)?,
            "created" => self.created = value.to_string(),
            "updated" => self.updated = value.to_string(),
            _ => return Err(FieldError::Unknown(field.to_string())),
        }
        Ok(())
    }
}

fn parse_numeric(field: &str, value: &str) -> Result<u32, FieldError> {
    value
        .parse::<u32>()
        .map_err(|_| FieldError::InvalidNumeric {
            field: field.to_string(),
            value: value.to_string(),
        })
}

/// Path to a slug's state file, relative to the current working directory
/// (which the CLI expects to be the repo root).
pub fn state_file_path(slug: &str) -> PathBuf {
    Path::new(".heist").join(slug).join("state.json")
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Casing,
    Planning,
    FenceReview,
    HumanReview,
    Forging,
    Safehouse,
    Implementing,
    Cleaning,
    Done,
}

impl Stage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::Casing => "casing",
            Stage::Planning => "planning",
            Stage::FenceReview => "fence_review",
            Stage::HumanReview => "human_review",
            Stage::Forging => "forging",
            Stage::Safehouse => "safehouse",
            Stage::Implementing => "implementing",
            Stage::Cleaning => "cleaning",
            Stage::Done => "done",
        }
    }

    pub fn parse(value: &str) -> Result<Stage, FieldError> {
        let stage = match value {
            "casing" => Stage::Casing,
            "planning" => Stage::Planning,
            "fence_review" => Stage::FenceReview,
            "human_review" => Stage::HumanReview,
            "forging" => Stage::Forging,
            "safehouse" => Stage::Safehouse,
            "implementing" => Stage::Implementing,
            "cleaning" => Stage::Cleaning,
            "done" => Stage::Done,
            _ => return Err(FieldError::InvalidStage(value.to_string())),
        };
        Ok(stage)
    }

    /// The `pipeline.md` step to resume at from this stage, as `(number, name)`.
    /// `Done` has no next step.
    pub fn next_step(&self) -> Option<(u32, &'static str)> {
        let step = match self {
            Stage::Casing => (1, "casing"),
            Stage::Planning => (2, "planning"),
            Stage::FenceReview => (3, "fence review"),
            Stage::HumanReview => (4, "human review"),
            Stage::Forging => (5, "forging"),
            Stage::Safehouse | Stage::Implementing => (6, "implementing"),
            Stage::Cleaning => (7, "cleaning"),
            Stage::Done => return None,
        };
        Some(step)
    }
}

/// Today's date as `YYYY-MM-DD` in UTC.
pub fn today() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        now.month() as u8,
        now.day()
    )
}

/// Failure loading or persisting a state file.
#[derive(Debug)]
pub enum StateError {
    Missing,
    AlreadyExists,
    Unreadable(std::io::Error),
    Unparseable(serde_json::Error),
    SchemaMismatch { found: u32, expected: u32 },
}

impl StateError {
    pub fn exit_code(&self) -> crate::exitcode::ExitCode {
        use crate::exitcode::ExitCode;
        match self {
            StateError::Missing => ExitCode::Precondition,
            StateError::AlreadyExists => ExitCode::Precondition,
            StateError::Unreadable(_) => ExitCode::Internal,
            StateError::Unparseable(_) => ExitCode::Precondition,
            StateError::SchemaMismatch { .. } => ExitCode::Precondition,
        }
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Missing => write!(f, "state file missing"),
            StateError::AlreadyExists => write!(f, "state directory already exists"),
            StateError::Unreadable(e) => write!(f, "failed to read state file: {}", e),
            StateError::Unparseable(e) => write!(f, "state file unparseable: {}", e),
            StateError::SchemaMismatch { found, expected } => write!(
                f,
                "schema version mismatch: file has version {}, but CLI supports version {}",
                found, expected
            ),
        }
    }
}

/// Failure addressing or parsing a single state field.
#[derive(Debug)]
pub enum FieldError {
    Unknown(String),
    InvalidStage(String),
    InvalidNumeric { field: String, value: String },
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldError::Unknown(field) => write!(f, "unknown field: {}", field),
            FieldError::InvalidStage(value) => write!(f, "invalid stage: {}", value),
            FieldError::InvalidNumeric { field, value } => write!(
                f,
                "invalid value for numeric field '{}': {} (expected an integer)",
                field, value
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_state_has_expected_defaults() {
        let today = today();
        let state = State::new("my-slug");
        let json = serde_json::to_value(&state).expect("failed to serialize");

        assert_eq!(
            json,
            json!({
                "schema_version": 1,
                "slug": "my-slug",
                "stage": "casing",
                "worktree": null,
                "branch": null,
                "score_step": 0,
                "score_steps_total": 0,
                "fence_rounds": 0,
                "created": today,
                "updated": today,
            })
        );
    }

    #[test]
    fn stage_variants_serialize_to_snake_case_strings() {
        let cases = [
            (Stage::Casing, "casing"),
            (Stage::Planning, "planning"),
            (Stage::FenceReview, "fence_review"),
            (Stage::HumanReview, "human_review"),
            (Stage::Forging, "forging"),
            (Stage::Safehouse, "safehouse"),
            (Stage::Implementing, "implementing"),
            (Stage::Cleaning, "cleaning"),
            (Stage::Done, "done"),
        ];
        for (stage, expected) in cases {
            assert_eq!(serde_json::to_value(stage).unwrap(), json!(expected));
        }
    }

    #[test]
    fn next_step_is_none_only_for_done() {
        assert_eq!(Stage::Forging.next_step(), Some((5, "forging")));
        assert_eq!(Stage::Safehouse.next_step(), Some((6, "implementing")));
        assert_eq!(Stage::Implementing.next_step(), Some((6, "implementing")));
        assert_eq!(Stage::Done.next_step(), None);
    }
}
