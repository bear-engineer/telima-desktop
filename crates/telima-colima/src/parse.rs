//! Pure parsers for `colima` CLI output. No I/O.
use telima_core::profile::{Profile, ProgressEvent};

/// Failure to read a piece of `colima` output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("line {line}: invalid JSON ({message})")]
    Json { line: usize, message: String },
    #[error("unknown runtime: {0:?}")]
    Runtime(String),
    #[error("invalid profile name: {0:?}")]
    Name(String),
    #[error("cannot read colima version from {0:?}")]
    Version(String),
}

/// Parse `colima ls -j` output (one JSON object per line).
pub fn parse_ls(_output: &str) -> Result<Vec<Profile>, ParseError> {
    todo!("RED: parse_ls")
}

/// Parse one stderr line of a long-running colima command.
pub fn parse_log_line(_line: &str) -> ProgressEvent {
    todo!("RED: parse_log_line")
}

/// Parse the first line of `colima version` output, e.g. `colima version 0.10.3`.
pub fn parse_version(_output: &str) -> Result<(u32, u32, u32), ParseError> {
    todo!("RED: parse_version")
}
