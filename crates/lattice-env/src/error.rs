use std::path::PathBuf;

/// Failure resolving a requested environment.
#[derive(Debug, thiserror::Error)]
pub enum EnvError {
    /// A required external tool (for example `uv` or `python3`) was not found.
    #[error("missing tool `{tool}` on PATH")]
    MissingTool { tool: String },

    /// The requested provider cannot be used right now (typed, not a silent fallback).
    #[error("environment unavailable: {reason}")]
    Unavailable { reason: String },

    /// Directory is not a uv project (no `pyproject.toml` or `uv.lock`).
    #[error("not a uv project (missing pyproject.toml and uv.lock): {path}")]
    NotAUvProject { path: PathBuf },

    /// An I/O error while discovering or invoking tools.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An external tool ran but returned a non-zero status or unusable output.
    #[error("tool `{tool}` failed: {detail}")]
    ToolFailed { tool: String, detail: String },
}
