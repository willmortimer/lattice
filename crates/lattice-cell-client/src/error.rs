//! Error types for the celld client.

use std::io;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, CellClientError>;

/// Failures talking to celld or building KernelFS plans.
#[derive(Debug, thiserror::Error)]
pub enum CellClientError {
    #[error("CELLD_BASE_URL is unset or empty; celld client refuses to guess a default")]
    MissingBaseUrl,

    #[error("http error: {0}")]
    Http(String),

    #[error("connect protocol error: {0}")]
    Connect(String),

    #[error("celld returned status {status}: {body}")]
    Status { status: u16, body: String },

    #[error("guest invoke error: {0}")]
    Invoke(String),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("path escape rejected: {0}")]
    PathEscape(String),

    #[error("invalid plan: {0}")]
    InvalidPlan(String),

    #[error("run task failed: state={state} exit_code={exit_code} detail={detail}")]
    RunTaskFailed {
        state: String,
        exit_code: i32,
        detail: String,
    },
}
