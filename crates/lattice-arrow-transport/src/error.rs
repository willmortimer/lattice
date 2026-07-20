use thiserror::Error;

/// Errors from Arrow conversion or IPC encode/decode.
#[derive(Debug, Error)]
pub enum Error {
    #[error("arrow error: {0}")]
    Arrow(String),
    #[error("transport cancelled")]
    Cancelled,
    #[error("{0}")]
    Message(String),
}

impl Error {
    pub(crate) fn arrow(message: impl Into<String>) -> Self {
        Self::Arrow(message.into())
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl From<arrow::error::ArrowError> for Error {
    fn from(value: arrow::error::ArrowError) -> Self {
        Self::Arrow(value.to_string())
    }
}
