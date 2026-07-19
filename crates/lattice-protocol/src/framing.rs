use bytes::{Bytes, BytesMut};
use prost::Message;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::error::ProtocolError;
use crate::Envelope;

/// Maximum accepted control-plane frame payload (length-delimited body).
pub const MAX_FRAME_LENGTH: usize = 16 * 1024 * 1024;

/// Build a length-delimited codec with Lattice control-plane limits.
pub fn length_delimited_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_FRAME_LENGTH)
        .length_field_length(4)
        .big_endian()
        .new_codec()
}

/// Encode an [`Envelope`] into a single length-delimited frame.
pub fn encode_frame(envelope: &Envelope) -> Result<Bytes, ProtocolError> {
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
///
/// Returns `Ok(None)` when the buffer holds only a partial frame. Does not
/// consume bytes until a complete frame is present, so callers can append and
/// retry with a fresh decode. Oversized frames and malformed protobuf fail
/// with [`ProtocolError`].
pub fn try_decode_frame(src: &mut BytesMut) -> Result<Option<Envelope>, ProtocolError> {
    let Some(declared_length) = peek_frame_length(src) else {
        return Ok(None);
    };
    if declared_length > MAX_FRAME_LENGTH {
        return Err(ProtocolError::FrameTooLarge {
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
///
/// Unlike feeding [`LengthDelimitedCodec`] directly, this does not strip the
/// length prefix until the full frame is buffered, so callers can append bytes
/// and retry safely.
#[derive(Debug, Default)]
pub struct FrameDecoder;

impl FrameDecoder {
    /// Create a decoder with Lattice control-plane frame limits.
    pub fn new() -> Self {
        Self
    }

    /// Decode the next complete envelope from `src`, if available.
    pub fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Envelope>, ProtocolError> {
        try_decode_frame(src)
    }
}

/// Decode exactly one complete frame from `src`.
///
/// Fails if the buffer is incomplete or contains trailing bytes after one frame.
pub fn decode_frame(src: &[u8]) -> Result<Envelope, ProtocolError> {
    let mut buf = BytesMut::from(src);
    match try_decode_frame(&mut buf)? {
        Some(envelope) if buf.is_empty() => Ok(envelope),
        Some(_) => Err(ProtocolError::Framing(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "trailing bytes after complete frame",
        ))),
        None => Err(ProtocolError::Framing(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "incomplete frame",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        request_envelope, response_envelope, HealthRequest, HealthResponse, PingRequest,
        PingResponse, Request, Response, PROTOCOL_VERSION,
    };

    fn health_request_envelope() -> Envelope {
        request_envelope(
            "req-1",
            Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(crate::request::Body::Health(HealthRequest {})),
            },
        )
    }

    #[test]
    fn round_trip_health_request() {
        let original = health_request_envelope();
        let framed = encode_frame(&original).expect("encode");
        let decoded = decode_frame(&framed).expect("decode");
        assert_eq!(decoded, original);
    }

    #[test]
    fn partial_frame_returns_none() {
        let framed = encode_frame(&health_request_envelope()).expect("encode");
        let mut partial = BytesMut::from(&framed[..framed.len().saturating_sub(1)]);
        assert!(try_decode_frame(&mut partial)
            .expect("partial decode")
            .is_none());
        assert!(!partial.is_empty());
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let declared = MAX_FRAME_LENGTH + 1;
        let mut src = BytesMut::new();
        src.extend_from_slice(&(declared as u32).to_be_bytes());
        src.extend_from_slice(&[0u8; 64]);

        let err = try_decode_frame(&mut src).expect_err("oversized");
        match err {
            ProtocolError::FrameTooLarge {
                max_frame_length,
                declared_length,
            } => {
                assert_eq!(max_frame_length, MAX_FRAME_LENGTH);
                assert_eq!(declared_length, declared);
            }
            other => panic!("expected FrameTooLarge, got {other:?}"),
        }
        assert!(
            err.to_string().contains("maximum length"),
            "error should be clear: {err}"
        );
    }

    #[test]
    fn ping_round_trip_preserves_nonce() {
        let request = request_envelope(
            "ping-1",
            Request {
                deadline_unix_ms: Some(1_700_000_000_000),
                idempotency_key: Some("idem-ping".into()),
                body: Some(crate::request::Body::Ping(PingRequest {
                    nonce: "abc".into(),
                })),
            },
        );
        let response = response_envelope(
            "ping-1",
            Response {
                body: Some(crate::response::Body::Ping(PingResponse {
                    nonce: "abc".into(),
                })),
            },
        );

        let req_bytes = encode_frame(&request).expect("encode request");
        let res_bytes = encode_frame(&response).expect("encode response");
        let decoded_req = decode_frame(&req_bytes).expect("decode request");
        let decoded_res = decode_frame(&res_bytes).expect("decode response");
        assert_eq!(decoded_req.protocol_version, PROTOCOL_VERSION);
        assert_eq!(decoded_res, response);

        match decoded_req.payload {
            Some(crate::envelope::Payload::Request(req)) => {
                assert_eq!(req.idempotency_key.as_deref(), Some("idem-ping"));
                match req.body {
                    Some(crate::request::Body::Ping(ping)) => assert_eq!(ping.nonce, "abc"),
                    other => panic!("expected ping body, got {other:?}"),
                }
            }
            other => panic!("expected request payload, got {other:?}"),
        }
    }

    #[test]
    fn health_response_round_trip() {
        let envelope = response_envelope(
            "health-1",
            Response {
                body: Some(crate::response::Body::Health(HealthResponse {
                    status: "ok".into(),
                    protocol_version: PROTOCOL_VERSION,
                    instance_id: "0190deadbeef".into(),
                    backend: None,
                })),
            },
        );
        let framed = encode_frame(&envelope).expect("encode");
        assert_eq!(decode_frame(&framed).expect("decode"), envelope);
    }

    #[test]
    fn stateful_decoder_handles_partial_then_complete() {
        let framed = encode_frame(&health_request_envelope()).expect("encode");
        let mut decoder = FrameDecoder::new();
        let split = framed.len() / 2;
        let mut buf = BytesMut::from(&framed[..split]);
        assert!(decoder.decode(&mut buf).expect("partial").is_none());
        buf.extend_from_slice(&framed[split..]);
        let decoded = decoder
            .decode(&mut buf)
            .expect("complete")
            .expect("envelope");
        assert_eq!(decoded.request_id, "req-1");
        assert!(buf.is_empty());
    }
}
