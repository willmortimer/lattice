use bytes::{Bytes, BytesMut};
use prost::Message;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::error::EmbedHostError;
use crate::Envelope;

/// Maximum accepted embed-host frame payload (length-delimited body).
pub const MAX_FRAME_LENGTH: usize = 16 * 1024 * 1024;

/// Build a length-delimited codec with embed-host limits.
pub fn length_delimited_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_FRAME_LENGTH)
        .length_field_length(4)
        .big_endian()
        .new_codec()
}

/// Encode an [`Envelope`] into a single length-delimited frame.
pub fn encode_frame(envelope: &Envelope) -> Result<Bytes, EmbedHostError> {
    let mut codec = length_delimited_codec();
    let mut dst = BytesMut::new();
    let payload = Bytes::from(envelope.encode_to_vec());
    codec.encode(payload, &mut dst)?;
    Ok(dst.freeze())
}

fn peek_frame_length(src: &BytesMut) -> Option<usize> {
    if src.len() < 4 {
        return None;
    }
    Some(u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize)
}

/// Attempt to decode one [`Envelope`] from a byte buffer.
pub fn try_decode_frame(src: &mut BytesMut) -> Result<Option<Envelope>, EmbedHostError> {
    let Some(declared_length) = peek_frame_length(src) else {
        return Ok(None);
    };
    if declared_length > MAX_FRAME_LENGTH {
        return Err(EmbedHostError::FrameTooLarge {
            max_frame_length: MAX_FRAME_LENGTH,
            declared_length,
        });
    }
    let frame_len = 4usize.saturating_add(declared_length);
    if src.len() < frame_len {
        return Ok(None);
    }

    let mut codec = length_delimited_codec();
    match codec.decode(src)? {
        Some(frame) => Ok(Some(Envelope::decode(frame)?)),
        None => Ok(None),
    }
}

/// Streaming-friendly decoder that waits for complete frames before consuming.
#[derive(Debug, Default)]
pub struct FrameDecoder;

impl FrameDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Envelope>, EmbedHostError> {
        try_decode_frame(src)
    }
}

/// Decode exactly one complete frame from `src`.
pub fn decode_frame(src: &[u8]) -> Result<Envelope, EmbedHostError> {
    let mut buf = BytesMut::from(src);
    match try_decode_frame(&mut buf)? {
        Some(envelope) if buf.is_empty() => Ok(envelope),
        Some(_) => Err(EmbedHostError::Framing(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "trailing bytes after complete frame",
        ))),
        None => Err(EmbedHostError::Framing(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "incomplete frame",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        request_envelope, response_envelope, HealthRequest, HealthResponse, Request, Response,
        PROTOCOL_VERSION,
    };

    fn health_request() -> Envelope {
        request_envelope(
            "req-1",
            Request {
                deadline_unix_ms: None,
                body: Some(crate::request::Body::Health(HealthRequest {})),
            },
        )
    }

    #[test]
    fn round_trip_health_request() {
        let original = health_request();
        let framed = encode_frame(&original).expect("encode");
        let decoded = decode_frame(&framed).expect("decode");
        assert_eq!(decoded, original);
        assert_eq!(decoded.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn health_response_round_trip() {
        let envelope = response_envelope(
            "health-1",
            Response {
                body: Some(crate::response::Body::Health(HealthResponse {
                    status: "ok".into(),
                    protocol_version: PROTOCOL_VERSION,
                    instance_id: "instance".into(),
                    backend: "fake".into(),
                })),
            },
        );
        let framed = encode_frame(&envelope).expect("encode");
        assert_eq!(decode_frame(&framed).expect("decode"), envelope);
    }
}
