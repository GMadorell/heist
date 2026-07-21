use crate::domain::error::ValueError;
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
    pub fn new(slug: &SlugValue, today: DateValue) -> Result<Self, ValueError> {
        Ok(State {
            schema_version: SchemaVersion::CURRENT,
            slug: slug.clone(),
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

    pub fn get_field(&self, cli_field: &str) -> Result<String, ValueError> {
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
            _ => return Err(ValueError::Unknown(cli_field.to_string())),
        };
        Ok(value)
    }

    pub fn set_field(&mut self, cli_field: &str, value: &str) -> Result<(), ValueError> {
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
            _ => return Err(ValueError::Unknown(cli_field.to_string())),
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
            Stage::Implementing => "implementing",
            Stage::Cleaning => "cleaning",
            Stage::Done => "done",
        }
    }

    pub fn parse(value: &str) -> Result<Stage, ValueError> {
        let stage = match value {
            "casing" => Stage::Casing,
            "planning" => Stage::Planning,
            "fence_review" => Stage::FenceReview,
            "human_review" => Stage::HumanReview,
            "forging" => Stage::Forging,
            "implementing" => Stage::Implementing,
            "cleaning" => Stage::Cleaning,
            "done" => Stage::Done,
            _ => return Err(ValueError::InvalidStage(value.to_string())),
        };
        Ok(stage)
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

    pub fn parse(value: &str) -> Result<Mode, ValueError> {
        match value {
            "heavy" => Ok(Mode::Heavy),
            "medium" => Ok(Mode::Medium),
            "light" => Ok(Mode::Light),
            _ => Err(ValueError::InvalidValue {
                field: "mode".to_string(),
                value: value.to_string(),
                expected: "one of: heavy, medium, light".to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineFile {
    Pipeline,
    PipelineStandard,
    PipelineLight,
}

impl PipelineFile {
    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineFile::Pipeline => "pipeline.md",
            PipelineFile::PipelineStandard => "pipeline-standard.md",
            PipelineFile::PipelineLight => "pipeline-light.md",
        }
    }
}

impl std::fmt::Display for PipelineFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Routing {
    pub file: PipelineFile,
    pub step: u8,
}

impl std::fmt::Display for Routing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} step {}", self.file, self.step)
    }
}

pub fn route(stage: Stage, mode: Mode) -> Option<Routing> {
    match stage {
        Stage::Casing => Some(Routing {
            file: PipelineFile::Pipeline,
            step: 1,
        }),
        Stage::Planning => Some(Routing {
            file: PipelineFile::Pipeline,
            step: 2,
        }),
        Stage::FenceReview => Some(Routing {
            file: PipelineFile::Pipeline,
            step: 3,
        }),
        Stage::HumanReview => Some(Routing {
            file: PipelineFile::Pipeline,
            step: 4,
        }),
        Stage::Forging => Some(Routing {
            file: PipelineFile::PipelineStandard,
            step: 5,
        }),
        Stage::Implementing => match mode {
            Mode::Light => Some(Routing {
                file: PipelineFile::PipelineLight,
                step: 2,
            }),
            Mode::Medium | Mode::Heavy => Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 6,
            }),
        },
        Stage::Cleaning => match mode {
            Mode::Light => Some(Routing {
                file: PipelineFile::PipelineLight,
                step: 3,
            }),
            Mode::Medium | Mode::Heavy => Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 7,
            }),
        },
        Stage::Done => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_state_has_expected_defaults() {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        let state = State::new(&SlugValue::parse("my-slug").expect("valid slug"), today.clone()).expect("valid slug");
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
    fn new_state_accepts_pre_validated_slug_value() {
        let today = DateValue::parse("today", "2026-01-01").expect("valid date");
        let state = State::new(&SlugValue::parse("my-slug").expect("valid slug"), today).expect("valid slug");
        assert_eq!(state.slug.to_string(), "my-slug");
    }

    #[test]
    fn stage_variants_serialize_to_snake_case_strings() {
        let cases = [
            (Stage::Casing, "casing"),
            (Stage::Planning, "planning"),
            (Stage::FenceReview, "fence_review"),
            (Stage::HumanReview, "human_review"),
            (Stage::Forging, "forging"),
            (Stage::Implementing, "implementing"),
            (Stage::Cleaning, "cleaning"),
            (Stage::Done, "done"),
        ];
        for (stage, expected) in cases {
            assert_eq!(serde_json::to_value(stage).unwrap(), json!(expected));
        }
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
            ValueError::InvalidValue { field, value, .. } => {
                assert_eq!(field, "mode");
                assert_eq!(value, "bogus");
            }
            _ => panic!("expected ValueError::InvalidValue, got a different variant"),
        }
    }

    #[test]
    fn route_maps_every_stage_mode_pair_to_its_doc_pointer() {
        assert_eq!(
            route(Stage::Casing, Mode::Heavy),
            Some(Routing {
                file: PipelineFile::Pipeline,
                step: 1
            })
        );
        assert_eq!(
            route(Stage::Planning, Mode::Light),
            Some(Routing {
                file: PipelineFile::Pipeline,
                step: 2
            })
        );
        assert_eq!(
            route(Stage::FenceReview, Mode::Heavy),
            Some(Routing {
                file: PipelineFile::Pipeline,
                step: 3
            })
        );
        assert_eq!(
            route(Stage::HumanReview, Mode::Medium),
            Some(Routing {
                file: PipelineFile::Pipeline,
                step: 4
            })
        );
        assert_eq!(
            route(Stage::Forging, Mode::Heavy),
            Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 5
            })
        );
        assert_eq!(
            route(Stage::Implementing, Mode::Medium),
            Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 6
            })
        );
        assert_eq!(
            route(Stage::Implementing, Mode::Heavy),
            Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 6
            })
        );
        assert_eq!(
            route(Stage::Implementing, Mode::Light),
            Some(Routing {
                file: PipelineFile::PipelineLight,
                step: 2
            })
        );
        assert_eq!(
            route(Stage::Cleaning, Mode::Heavy),
            Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 7
            })
        );
        assert_eq!(
            route(Stage::Cleaning, Mode::Medium),
            Some(Routing {
                file: PipelineFile::PipelineStandard,
                step: 7
            })
        );
        assert_eq!(
            route(Stage::Cleaning, Mode::Light),
            Some(Routing {
                file: PipelineFile::PipelineLight,
                step: 3
            })
        );
        assert_eq!(route(Stage::Done, Mode::Heavy), None);
    }
}
