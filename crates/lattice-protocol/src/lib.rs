//! Versioned Protobuf IPC contract for Lattice daemon clients (ADR 0041).
//!
//! Frames are length-delimited Protobuf [`Envelope`] messages. Domain payloads
//! include Health/Ping, OpenWorkspace/Search, ApplyPageUpdate (D3 one-writer
//! mutations), and sequenced ResourceChanged / IndexProgress events (D4).

mod error;
mod framing;

pub use error::ProtocolError;
pub use framing::{
    decode_frame, encode_frame, length_delimited_codec, try_decode_frame, FrameDecoder,
    MAX_FRAME_LENGTH,
};

/// Wire protocol version carried in every [`Envelope`].
pub const PROTOCOL_VERSION: u32 = 1;

include!(concat!(env!("OUT_DIR"), "/lattice.v1.rs"));

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

/// Build an event envelope with the current [`PROTOCOL_VERSION`].
pub fn event_envelope(request_id: impl Into<String>, event: Event) -> Envelope {
    Envelope {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.into(),
        payload: Some(envelope::Payload::Event(event)),
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
