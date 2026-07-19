use std::io;

use lattice_protocol::ProtocolError;
use lattice_voice::SpeechError;

/// Errors from voice-host framing, RPC, or backend operations.
#[derive(Debug, thiserror::Error)]
pub enum VoiceHostError {
    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("host error {code}: {message}")]
    Remote { code: String, message: String },

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error(transparent)]
    Framing(#[from] ProtocolError),

    #[error("speech error: {0}")]
    Speech(#[from] SpeechError),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("model not prepared")]
    ModelNotPrepared,
}

impl VoiceHostError {
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
