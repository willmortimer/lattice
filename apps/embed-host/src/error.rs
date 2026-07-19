use std::io;

/// Errors from embed-host framing, RPC, or backend operations.
#[derive(Debug, thiserror::Error)]
pub enum EmbedHostError {
    #[error(
        "frame exceeds maximum length of {max_frame_length} bytes (declared {declared_length})"
    )]
    FrameTooLarge {
        max_frame_length: usize,
        declared_length: usize,
    },

    #[error("framing error: {0}")]
    Framing(#[source] io::Error),

    #[error("invalid envelope protobuf: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("host error {code}: {message}")]
    Remote { code: String, message: String },

    #[error("io error: {0}")]
    Io(io::Error),

    #[error("embedding error: {0}")]
    Embedding(#[from] lattice_embedding::EmbeddingError),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("cancelled")]
    Cancelled,

    #[error("model not loaded")]
    ModelNotLoaded,
}

impl From<io::Error> for EmbedHostError {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::InvalidData {
            let message = err.to_string();
            if message.contains("frame size too big") || message.contains("max frame length") {
                return Self::FrameTooLarge {
                    max_frame_length: crate::framing::MAX_FRAME_LENGTH,
                    declared_length: 0,
                };
            }
        }
        // Connection / socket failures surface as Io; codec failures as Framing.
        if matches!(
            err.kind(),
            io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionRefused
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::NotConnected
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::UnexpectedEof
        ) {
            return Self::Io(err);
        }
        Self::Framing(err)
    }
}

impl EmbedHostError {
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(message.into())
    }

    pub fn remote(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Remote {
            code: code.into(),
            message: message.into(),
        }
    }
}
