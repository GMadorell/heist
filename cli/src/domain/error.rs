use std::fmt;

#[derive(Debug)]
pub enum StateError {
    Missing,
    AlreadyExists,
    Unreadable(std::io::Error),
    Unparseable(serde_json::Error),
}

impl StateError {
    pub fn exit_code(&self) -> crate::exitcode::ExitCode {
        use crate::exitcode::ExitCode;
        match self {
            StateError::Missing => ExitCode::Precondition,
            StateError::AlreadyExists => ExitCode::Precondition,
            StateError::Unreadable(_) => ExitCode::Internal,
            StateError::Unparseable(_) => ExitCode::Precondition,
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
        }
    }
}

#[derive(Debug)]
pub enum FieldError {
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
            FieldError::InvalidValue {
                field,
                value,
                expected,
            } => write!(
                f,
                "invalid value for field '{}': {} (expected {})",
                field, value, expected
            ),
        }
    }
}
