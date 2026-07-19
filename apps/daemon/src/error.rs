use thiserror::Error;

/// Errors from daemon listen/serve/spawn helpers.
#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(#[from] lattice_protocol::ProtocolError),

    #[error("runtime error: {0}")]
    Runtime(#[from] lattice_runtime::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("handshake rejected")]
    HandshakeRejected,

    #[error("spawn failed: {0}")]
    Spawn(String),

    #[error("timed out waiting for latticed readiness at {path}")]
    ReadyTimeout { path: String },

    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;
