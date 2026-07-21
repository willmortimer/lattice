//! Errors from the out-of-process kernel supervisor.

use std::path::PathBuf;

/// Errors produced by [`crate::KernelSessionMap`] and related helpers.
#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    /// Working directory is missing, not a directory, or escapes the workspace root.
    #[error("cwd is not allowed under workspace root: {cwd} (root: {workspace_root})")]
    CwdNotAllowed {
        cwd: PathBuf,
        workspace_root: PathBuf,
    },

    /// Neither `uv` nor `python3` could be found on `PATH`.
    #[error("no Python launcher found (looked for `uv` then `python3` on PATH)")]
    PythonNotFound,

    /// Spawning or talking to the bridge process failed.
    #[error("kernel bridge spawn failed: {message}")]
    SpawnFailed { message: String },

    /// The bridge exited or closed its pipes.
    #[error("kernel session is no longer alive")]
    DeadSession,

    /// No session is registered under the given id.
    #[error("unknown kernel session: {session_id}")]
    UnknownSession { session_id: String },

    /// Protocol framing or JSON decode failed.
    #[error("kernel protocol error: {message}")]
    Protocol { message: String },

    /// Timed out waiting for a bridge response.
    #[error("timed out waiting for kernel response")]
    Timeout,

    /// I/O against the bridge stdio pipes failed.
    #[error("kernel I/O error: {message}")]
    Io { message: String },
}

impl KernelError {
    pub(crate) fn spawn(err: impl std::fmt::Display) -> Self {
        Self::SpawnFailed {
            message: err.to_string(),
        }
    }

    pub(crate) fn protocol(err: impl std::fmt::Display) -> Self {
        Self::Protocol {
            message: err.to_string(),
        }
    }

    pub(crate) fn io(err: impl std::fmt::Display) -> Self {
        Self::Io {
            message: err.to_string(),
        }
    }
}
