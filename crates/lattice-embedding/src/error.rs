/// Errors produced by embedding providers and manifest verification.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EmbeddingError {
    /// The provider rejected the request.
    #[error("provider error: {message}")]
    Provider { message: String },

    /// The requested dimensions are incompatible with the provider specification.
    #[error("invalid dimensions {requested}: provider supports {supported}")]
    InvalidDimensions { requested: u32, supported: u32 },

    /// A model manifest failed validation.
    #[error("manifest error: {message}")]
    Manifest { message: String },

    /// A model artifact failed sha256 verification.
    #[error("artifact sha256 mismatch: expected {expected}, got {actual}")]
    ArtifactSha256Mismatch { expected: String, actual: String },

    /// The model artifact is not present on disk.
    #[error("artifact not found: {path}")]
    ArtifactNotFound { path: String },
}

impl EmbeddingError {
    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider {
            message: message.into(),
        }
    }

    pub fn manifest(message: impl Into<String>) -> Self {
        Self::Manifest {
            message: message.into(),
        }
    }
}
