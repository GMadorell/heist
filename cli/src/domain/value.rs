use crate::domain::error::FieldError;
use nutype::nutype;
use serde::{Deserialize, Serialize};
use std::fmt;
use time::macros::format_description;
use time::Date;

#[nutype(
    validate(predicate = SlugValue::is_valid_slug),
    derive(Debug, Clone, PartialEq, Eq, AsRef, Serialize, Deserialize, Display)
)]
pub struct SlugValue(String);

impl SlugValue {
    pub fn parse(value: &str) -> Result<Self, FieldError> {
        Self::try_new(value.to_string()).map_err(|_| FieldError::InvalidValue {
            field: "slug".to_string(),
            value: value.to_string(),
            expected: "kebab-case: lowercase letters, digits, single hyphens".to_string(),
        })
    }

    fn is_valid_slug(s: &str) -> bool {
        !s.is_empty()
            && !s.starts_with('-')
            && !s.ends_with('-')
            && !s.contains("--")
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }
}

#[nutype(
    validate(predicate = |s: &str| !s.trim().is_empty()),
    derive(Debug, Clone, PartialEq, Eq, AsRef, Serialize, Deserialize, Display)
)]
pub struct NonBlankValue(String);

impl NonBlankValue {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        Self::try_new(value.to_string()).map_err(|_| FieldError::InvalidValue {
            field: field.to_string(),
            value: value.to_string(),
            expected: "a non-blank string".to_string(),
        })
    }
}

#[nutype(
    validate(predicate = DateValue::is_valid_date),
    derive(Debug, Clone, PartialEq, Eq, AsRef, Serialize, Deserialize, Display)
)]
pub struct DateValue(String);

impl DateValue {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        Self::try_new(value.to_string()).map_err(|_| FieldError::InvalidValue {
            field: field.to_string(),
            value: value.to_string(),
            expected: "an ISO 8601 date (YYYY-MM-DD)".to_string(),
        })
    }

    fn is_valid_date(s: &str) -> bool {
        Date::parse(s, format_description!("[year]-[month]-[day]")).is_ok()
    }
}

#[nutype(derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsRef,
    Serialize,
    Deserialize,
    Display
))]
pub struct ScoreStep(u32);

impl ScoreStep {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        parse_numeric(field, value, Self::new)
    }
}

#[nutype(derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsRef,
    Serialize,
    Deserialize,
    Display
))]
pub struct ScoreStepsTotal(u32);

impl ScoreStepsTotal {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        parse_numeric(field, value, Self::new)
    }
}

#[nutype(derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsRef,
    Serialize,
    Deserialize,
    Display
))]
pub struct ScoreWave(u32);

impl ScoreWave {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        parse_numeric(field, value, Self::new)
    }
}

#[nutype(derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsRef,
    Serialize,
    Deserialize,
    Display
))]
pub struct ScoreWavesTotal(u32);

impl ScoreWavesTotal {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        parse_numeric(field, value, Self::new)
    }
}

#[nutype(derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsRef,
    Serialize,
    Deserialize,
    Display
))]
pub struct FenceRounds(u32);

impl FenceRounds {
    pub fn parse(field: &str, value: &str) -> Result<Self, FieldError> {
        parse_numeric(field, value, Self::new)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum SchemaVersion {
    V1,
}

impl SchemaVersion {
    pub const CURRENT: SchemaVersion = SchemaVersion::V1;

    pub fn parse(value: &str) -> Result<Self, FieldError> {
        let raw: u32 = value.parse().map_err(|_| FieldError::InvalidNumeric {
            field: "schema_version".to_string(),
            value: value.to_string(),
        })?;
        SchemaVersion::try_from(raw).map_err(|_| FieldError::InvalidValue {
            field: "schema_version".to_string(),
            value: value.to_string(),
            expected: format!(
                "a supported schema version (currently {})",
                SchemaVersion::CURRENT
            ),
        })
    }

    fn as_u32(&self) -> u32 {
        match self {
            SchemaVersion::V1 => 1,
        }
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u32())
    }
}

impl From<SchemaVersion> for u32 {
    fn from(version: SchemaVersion) -> u32 {
        version.as_u32()
    }
}

impl TryFrom<u32> for SchemaVersion {
    type Error = UnsupportedSchemaVersion;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SchemaVersion::V1),
            other => Err(UnsupportedSchemaVersion(other)),
        }
    }
}

#[derive(Debug)]
pub struct UnsupportedSchemaVersion(u32);

impl fmt::Display for UnsupportedSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "schema version mismatch: file has version {}, but CLI supports version {}",
            self.0,
            SchemaVersion::CURRENT.as_u32()
        )
    }
}

fn parse_numeric<T>(
    field: &str,
    value: &str,
    ctor: impl FnOnce(u32) -> T,
) -> Result<T, FieldError> {
    value
        .parse::<u32>()
        .map(ctor)
        .map_err(|_| FieldError::InvalidNumeric {
            field: field.to_string(),
            value: value.to_string(),
        })
}
