//! First-frame connection handshake for [`crate::DaemonClient`].
//!
//! Handshake frames use the same length-delimited codec as control envelopes
//! but carry dedicated prost messages (not [`lattice_protocol::Envelope`]).
//! The real daemon will share this convention in D2; D0 keeps the types here
//! so `lattice-protocol` stays limited to control-plane envelopes.

use bytes::{Bytes, BytesMut};
use lattice_protocol::{length_delimited_codec, ProtocolError, PROTOCOL_VERSION};
use prost::Message;
use tokio_util::codec::{Decoder, Encoder};

/// Client → daemon handshake payload.
#[derive(Clone, PartialEq, Eq, Message)]
pub struct HandshakeRequest {
    #[prost(uint32, tag = "1")]
    pub protocol_version: u32,
    #[prost(string, tag = "2")]
    pub auth_token: String,
}

/// Daemon → client handshake reply.
#[derive(Clone, PartialEq, Eq, Message)]
pub struct HandshakeResponse {
    #[prost(bool, tag = "1")]
    pub accepted: bool,
    #[prost(uint32, tag = "2")]
    pub protocol_version: u32,
    #[prost(string, tag = "3")]
    pub instance_id: String,
    #[prost(string, tag = "4")]
    pub message: String,
}

impl HandshakeRequest {
    /// Build a handshake for the current [`PROTOCOL_VERSION`].
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            auth_token: auth_token.into(),
        }
    }
}

/// Encode a handshake message as one length-delimited frame.
pub fn encode_handshake_frame<M: Message>(message: &M) -> Result<Bytes, ProtocolError> {
    let mut codec = length_delimited_codec();
    let mut dst = BytesMut::new();
    let payload = Bytes::from(message.encode_to_vec());
    codec.encode(payload, &mut dst)?;
    Ok(dst.freeze())
}

/// Decode exactly one complete handshake frame from `src`.
pub fn decode_handshake_frame<M: Message + Default>(src: &[u8]) -> Result<M, ProtocolError> {
    let mut buf = BytesMut::from(src);
    let mut codec = length_delimited_codec();
    match codec.decode(&mut buf)? {
        Some(frame) if buf.is_empty() => Ok(M::decode(frame)?),
        Some(_) => Err(ProtocolError::Framing(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "trailing bytes after complete handshake frame",
        ))),
        None => Err(ProtocolError::Framing(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "incomplete handshake frame",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_round_trip() {
        let request = HandshakeRequest::new("secret-token");
        let framed = encode_handshake_frame(&request).expect("encode");
        let decoded: HandshakeRequest = decode_handshake_frame(&framed).expect("decode");
        assert_eq!(decoded, request);
        assert_eq!(decoded.protocol_version, PROTOCOL_VERSION);
    }
}
