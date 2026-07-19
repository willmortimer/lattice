use std::path::PathBuf;

/// Errors produced by the PTY session core.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    /// `cwd` is missing or is not a directory.
    #[error("working directory does not exist or is not a directory: {path}")]
    InvalidCwd { path: PathBuf },

    /// Opening the PTY or spawning the shell failed.
    #[error("failed to spawn terminal session: {message}")]
    SpawnFailed { message: String },

    /// The session has already exited or been killed.
    #[error("terminal session is no longer alive")]
    DeadSession,

    /// Read/write/resize I/O against the PTY failed.
    #[error("terminal I/O error: {message}")]
    Io { message: String },
}

impl TerminalError {
    pub(crate) fn spawn(err: impl std::fmt::Display) -> Self {
        Self::SpawnFailed {
            message: err.to_string(),
        }
    }

    pub(crate) fn io(err: impl std::fmt::Display) -> Self {
        Self::Io {
            message: err.to_string(),
        }
    }
}
