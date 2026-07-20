//! Isolated embedding inference host for Lattice.
//!
//! The host listens on a private Unix-domain socket and speaks a length-
//! delimited Protobuf protocol. The default `fake` backend is always available
//! for CI. The optional `llama-cpp` feature links llama.cpp + Metal and loads
//! verified Qwen3 GGUF artifacts for real 512-d embeddings.

mod backend;
mod client;
mod error;
mod framing;
mod install;
mod server;
mod spec;

pub use backend::BackendKind;
pub use client::{
    socket_path_in, EmbedHostClient, EmbedHostSession, ReconnectableEmbedHostProvider,
};
pub use error::EmbedHostError;
pub use framing::{
    decode_frame, encode_frame, try_decode_frame, FrameDecoder, MAX_FRAME_LENGTH,
};
pub use install::{install_model, InstallResult};
pub use server::{run_server, HostConfig, HostState};
pub use spec::{embedding_spec_from_proto, embedding_spec_to_proto};

/// Wire protocol version for embed-host envelopes.
pub const PROTOCOL_VERSION: u32 = 1;

include!(concat!(env!("OUT_DIR"), "/lattice.embed_host.v1.rs"));

/// Build a request envelope with the current [`PROTOCOL_VERSION`].
pub fn request_envelope(request_id: impl Into<String>, request: Request) -> Envelope {
    Envelope {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.into(),
        payload: Some(envelope::Payload::Request(request)),
    }
}

/// Build a response envelope with the current [`PROTOCOL_VERSION`].
pub fn response_envelope(request_id: impl Into<String>, response: Response) -> Envelope {
    Envelope {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.into(),
        payload: Some(envelope::Payload::Response(response)),
    }
}

/// Build an error envelope with the current [`PROTOCOL_VERSION`].
pub fn error_envelope(request_id: impl Into<String>, error: Error) -> Envelope {
    Envelope {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.into(),
        payload: Some(envelope::Payload::Error(error)),
    }
}
