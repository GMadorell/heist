use crate::domain::error::FieldError;
use crate::domain::value::{
    DateValue, FenceRounds, NonBlankValue, SchemaVersion, ScoreStepsTotal, ScoreWave,
    ScoreWavesTotal, SlugValue,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub schema_version: SchemaVersion,
    pub slug: SlugValue,
    pub stage: Stage,
    pub mode: Mode,
    pub worktree: Option<NonBlankValue>,
    pub branch: Option<NonBlankValue>,
    pub base: Option<NonBlankValue>,
    pub score_wave: ScoreWave,
    pub score_waves_total: ScoreWavesTotal,
    pub score_steps_total: ScoreStepsTotal,
    pub fence_rounds: FenceRounds,
    pub created: DateValue,
    pub updated: DateValue,
}

impl State {
    pub fn new(slug: &str, today: DateValue) -> Result<Self, FieldError> {
        Ok(State {
            schema_version: SchemaVersion::CURRENT,
            slug: SlugValue::parse(slug)?,
            stage: Stage::Casing,
            mode: Mode::default(),
            worktree: None,
            branch: None,
            base: None,
            score_wave: ScoreWave::new(0),
            score_waves_total: ScoreWavesTotal::new(0),
            score_steps_total: ScoreStepsTotal::new(0),
            fence_rounds: FenceRounds::new(0),
            created: today.clone(),
            updated: today,
        })
    }

    pub fn get_field(&self, cli_field: &str) -> Result<String, FieldError> {
        let value = match cli_field {
            "schema_version" => self.schema_version.to_string(),
            "slug" => self.slug.to_string(),
            "stage" => self.stage.as_str().to_string(),
            "mode" => self.mode.as_str().to_string(),
            "worktree" => self
                .worktree
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string()),
            "branch" => self
                .branch
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string()),
            "base" => self
                .base
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string()),
            "score_wave" => self.score_wave.to_string(),
            "score_waves_total" => self.score_waves_total.to_string(),
            "score_steps_total" => self.score_steps_total.to_string(),
            "fence_rounds" => self.fence_rounds.to_string(),
            "created" => self.created.to_string(),
            "updated" => self.updated.to_string(),
            _ => return Err(FieldError::Unknown(cli_field.to_string())),
        };
        Ok(value)
    }

    pub fn set_field(&mut self, cli_field: &str, value: &str) -> Result<(), FieldError> {
        match cli_field {
            "schema_version" => self.schema_version = SchemaVersion::parse(value)?,
            "slug" => self.slug = SlugValue::parse(value)?,
            "stage" => self.stage = Stage::parse(value)?,
            "mode" => self.mode = Mode::parse(value)?,
            "worktree" => self.worktree = Some(NonBlankValue::parse(cli_field, value)?),
            "branch" => self.branch = Some(NonBlankValue::parse(cli_field, value)?),
            "base" => self.base = Some(NonBlankValue::parse(cli_field, value)?),
            "score_wave" => self.score_wave = ScoreWave::parse(cli_field, value)?,
            "score_waves_total" => {
                self.score_waves_total = ScoreWavesTotal::parse(cli_field, value)?
            }
            "score_steps_total" => {
                self.score_steps_total = ScoreStepsTotal::parse(cli_field, value)?
            }
            "fence_rounds" => self.fence_rounds = FenceRounds::parse(cli_field, value)?,
            "created" => self.created = DateValue::parse(cli_field, value)?,
            "updated" => self.updated = DateValue::parse(cli_field, value)?,
            _ => return Err(FieldError::Unknown(cli_field.to_string())),
        }
        Ok(())
    }
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

    pub fn next_step(&self) -> Option<Stage> {
        match self {
            Stage::Safehouse => Some(Stage::Implementing),
            Stage::Done => None,
            other => Some(*other),
        }
    }
}

/// How much of the pipeline runs: `heavy` is the full pipeline, `medium` skips
/// Fence review, `light` skips Fence, Forger, Wheelman, and the Cleaner in favor
/// of direct implementation with a manual crit review of the diff.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Heavy,
    Medium,
    Light,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Heavy => "heavy",
            Mode::Medium => "medium",
            Mode::Light => "light",
        }
    }

    pub fn parse(value: &str) -> Result<Mode, FieldError> {
        match value {
            "heavy" => Ok(Mode::Heavy),
            "medium" => Ok(Mode::Medium),
            "light" => Ok(Mode::Light),
            _ => Err(FieldError::InvalidValue {
                field: "mode".to_string(),
                value: value.to_string(),
                expected: "one of: heavy, medium, light".to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_state_has_expected_defaults() {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        let state = State::new("my-slug", today.clone()).expect("valid slug");
        let json = serde_json::to_value(&state).expect("failed to serialize");

        assert_eq!(
            json,
            json!({
                "schema_version": 1,
                "slug": "my-slug",
                "stage": "casing",
                "mode": "heavy",
                "worktree": null,
                "branch": null,
                "base": null,
                "score_wave": 0,
                "score_waves_total": 0,
                "score_steps_total": 0,
                "fence_rounds": 0,
                "created": today.to_string(),
                "updated": today.to_string(),
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
        assert_eq!(Stage::Forging.next_step(), Some(Stage::Forging));
        assert_eq!(Stage::Safehouse.next_step(), Some(Stage::Implementing));
        assert_eq!(Stage::Implementing.next_step(), Some(Stage::Implementing));
        assert_eq!(Stage::Done.next_step(), None);
    }

    #[test]
    fn mode_variants_serialize_to_snake_case_strings() {
        let cases = [
            (Mode::Heavy, "heavy"),
            (Mode::Medium, "medium"),
            (Mode::Light, "light"),
        ];
        for (mode, expected) in cases {
            assert_eq!(serde_json::to_value(mode).unwrap(), json!(expected));
        }
    }

    #[test]
    fn mode_defaults_to_heavy() {
        assert_eq!(Mode::default(), Mode::Heavy);
    }

    #[test]
    fn mode_parse_rejects_unknown_value() {
        let err = Mode::parse("bogus").expect_err("should reject unknown mode");
        match err {
            FieldError::InvalidValue { field, value, .. } => {
                assert_eq!(field, "mode");
                assert_eq!(value, "bogus");
            }
            _ => panic!("expected FieldError::InvalidValue, got a different variant"),
        }
    }
}
