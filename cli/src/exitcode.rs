/// The CLI's exit-code contract, shared by every command.
///
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
