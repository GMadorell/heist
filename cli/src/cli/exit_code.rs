use crate::domain::error::StateError;
use crate::domain::validation::ValidationError;
use crate::ports::git::GitError;

/// The discriminants are the raw process exit codes callers rely on, so they
/// are part of the public contract and must not be renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0,
    Internal = 1,
    Precondition = 2,
    Git = 3,
}

impl ExitCode {
    /// Terminate the process with this exit code.
    pub fn exit(self) -> ! {
        std::process::exit(self as i32)
    }
}

impl From<&StateError> for ExitCode {
    fn from(e: &StateError) -> Self {
        match e {
            StateError::Missing => ExitCode::Precondition,
            StateError::AlreadyExists => ExitCode::Precondition,
            StateError::Unreadable(_) => ExitCode::Internal,
            StateError::Unparseable(_) => ExitCode::Precondition,
        }
    }
}

impl From<&GitError> for ExitCode {
    fn from(_: &GitError) -> Self {
        ExitCode::Git
    }
}

impl From<&ValidationError> for ExitCode {
    fn from(e: &ValidationError) -> Self {
        match e {
            ValidationError::PathNotAbsolute { .. } => ExitCode::Precondition,
            ValidationError::PathOutsideProject { .. } => ExitCode::Precondition,
            ValidationError::Other(_) => ExitCode::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn path_outside_project_maps_to_precondition() {
        let e = ValidationError::PathOutsideProject {
            requested: PathBuf::from("x"),
            project_root: PathBuf::from("/y"),
        };
        assert_eq!(ExitCode::from(&e), ExitCode::Precondition);
    }

    #[test]
    fn other_maps_to_internal() {
        let e = ValidationError::Other("boom".into());
        assert_eq!(ExitCode::from(&e), ExitCode::Internal);
    }
}
