use lattice_protocol::{Error as ProtocolWireError, ProtocolError};
use std::io;

/// Errors from [`crate::LatticeClient`] operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The peer rejected the connection handshake.
    #[error("handshake rejected: {message}")]
    HandshakeRejected { message: String },

    /// Local protocol version is incompatible with the peer.
    #[error("protocol version mismatch: client={client_version}, peer={peer_version}")]
    ProtocolVersionMismatch {
        client_version: u32,
        peer_version: u32,
    },

    /// Length-delimited framing or envelope decode failed.
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    /// I/O failure on the daemon socket or embedded transport.
    #[error("transport error: {0}")]
    Transport(#[from] io::Error),

    /// The peer returned a structured protocol error envelope.
    #[error("remote error {code}: {message}")]
    Remote {
        code: String,
        message: String,
        details: Option<String>,
    },

    /// Response body was missing or did not match the request.
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),

    /// Feature is intentionally stubbed until a later migration phase.
    #[error("unimplemented: {0}")]
    Unimplemented(&'static str),
}

impl ClientError {
    /// Map a wire [`Error`](ProtocolWireError) payload into [`ClientError::Remote`].
    pub fn from_wire(error: ProtocolWireError) -> Self {
        Self::Remote {
            code: error.code,
            message: error.message,
            details: error.details,
        }
    }
}
