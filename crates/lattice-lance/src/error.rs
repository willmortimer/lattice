/// Errors produced by Lance-backed multimodal store operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LanceError {
    /// The operation is not implemented yet.
    #[error("not implemented: {message}")]
    NotImplemented { message: String },

    /// The request or row payload failed validation.
    #[error("invalid input: {message}")]
    InvalidInput { message: String },

    /// The dataset or workspace path is missing or inaccessible.
    #[error("dataset not found: {path}")]
    DatasetNotFound { path: String },

    /// An underlying I/O failure occurred.
    #[error("io error at {path}: {message}")]
    Io { path: String, message: String },
}

pub type Result<T> = std::result::Result<T, LanceError>;

impl LanceError {
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::NotImplemented {
            message: message.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
}
