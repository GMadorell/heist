use std::fmt;

#[derive(Debug)]
pub enum StateError {
    Missing,
    AlreadyExists,
    Unreadable(std::io::Error),
    Unparseable(serde_json::Error),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Missing => write!(f, "state file missing"),
            StateError::AlreadyExists => write!(f, "state directory already exists"),
            StateError::Unreadable(e) => write!(f, "failed to read state file: {}", e),
            StateError::Unparseable(e) => write!(f, "state file unparseable: {}", e),
        }
    }
}

#[derive(Debug)]
pub enum ValueError {
    Unknown(String),
    InvalidStage(String),
    InvalidNumeric {
        field: String,
        value: String,
    },
    InvalidValue {
        field: String,
        value: String,
        expected: String,
    },
    NotIncrementable(String),
    InvalidRef {
        value: String,
        reason: String,
    },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueError::Unknown(field) => write!(f, "unknown field: {}", field),
            ValueError::InvalidStage(value) => write!(f, "invalid stage: {}", value),
            ValueError::InvalidNumeric { field, value } => write!(
                f,
                "invalid value for numeric field '{}': {} (expected an integer)",
                field, value
            ),
            ValueError::InvalidValue {
                field,
                value,
                expected,
            } => write!(
                f,
                "invalid value for field '{}': {} (expected {})",
                field, value, expected
            ),
            ValueError::NotIncrementable(field) => {
                write!(f, "field '{}' is not incrementable (not a number)", field)
            }
            ValueError::InvalidRef { value, reason } => {
                write!(f, "invalid ref '{}': {}", value, reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_incrementable_display_names_the_field() {
        let err = ValueError::NotIncrementable("stage".to_string());
        assert_eq!(
            err.to_string(),
            "field 'stage' is not incrementable (not a number)"
        );
    }
}
