use bytes::BytesMut;
use lattice_protocol::{
    decode_frame, encode_frame, request_envelope, try_decode_frame, AudioSampleFormat,
    HealthRequest, ProtocolError, PushAudioChunkRequest, Request, MAX_FRAME_LENGTH,
};

fn sample_frame() -> bytes::Bytes {
    let envelope = request_envelope(
        "framing",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::Health(HealthRequest {})),
        },
    );
    encode_frame(&envelope).expect("encode sample")
}

fn push_audio_frame() -> bytes::Bytes {
    let payload: Vec<u8> = [0.25f32.to_le_bytes(), (-0.25f32).to_le_bytes()]
        .into_iter()
        .flatten()
        .collect();
    let envelope = request_envelope(
        "framing-audio",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::PushAudioChunk(
                PushAudioChunkRequest {
                    session_id: "vs-frame".into(),
                    sequence: 7,
                    captured_at_ns: 99,
                    sample_rate_hz: 16_000,
                    channels: 1,
                    sample_format: AudioSampleFormat::F32 as i32,
                    payload,
                },
            )),
        },
    );
    encode_frame(&envelope).expect("encode audio")
}

#[test]
fn partial_length_prefix_waits() {
    let frame = sample_frame();
    let mut buf = BytesMut::from(&frame[..2]);
    assert!(try_decode_frame(&mut buf).expect("decode").is_none());
}

#[test]
fn partial_payload_waits() {
    let frame = sample_frame();
    assert!(frame.len() > 5);
    let mut buf = BytesMut::from(&frame[..frame.len() - 3]);
    assert!(try_decode_frame(&mut buf).expect("decode").is_none());
}

#[test]
fn split_then_complete_decodes() {
    let frame = sample_frame();
    let split = frame.len() / 2;
    let mut buf = BytesMut::from(&frame[..split]);
    assert!(try_decode_frame(&mut buf).expect("first").is_none());
    buf.extend_from_slice(&frame[split..]);
    let decoded = try_decode_frame(&mut buf)
        .expect("second")
        .expect("complete frame");
    assert_eq!(decoded.request_id, "framing");
    assert!(buf.is_empty());
}

#[test]
fn oversized_declared_length_rejects_clearly() {
    let declared = MAX_FRAME_LENGTH + 8;
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&(declared as u32).to_be_bytes());
    buf.extend_from_slice(&[0xAB; 32]);

    let err = try_decode_frame(&mut buf).expect_err("must reject oversized");
    let message = err.to_string();
    assert!(
        message.contains("maximum length") || matches!(err, ProtocolError::FrameTooLarge { .. }),
        "unexpected error: {message}"
    );
    match err {
        ProtocolError::FrameTooLarge {
            max_frame_length,
            declared_length,
        } => {
            assert_eq!(max_frame_length, MAX_FRAME_LENGTH);
            assert_eq!(declared_length, declared);
        }
        other => panic!("expected FrameTooLarge, got {other}"),
    }
}

#[test]
fn voice_push_audio_chunk_round_trips_through_framing() {
    let frame = push_audio_frame();
    let decoded = decode_frame(&frame).expect("decode audio frame");
    assert_eq!(decoded.request_id, "framing-audio");
    let Some(lattice_protocol::envelope::Payload::Request(req)) = decoded.payload else {
        panic!("expected request payload");
    };
    let Some(lattice_protocol::request::Body::PushAudioChunk(chunk)) = req.body else {
        panic!("expected push audio chunk");
    };
    assert_eq!(chunk.sequence, 7);
    assert_eq!(chunk.captured_at_ns, 99);
    assert_eq!(chunk.payload.len(), 8);
    assert_eq!(chunk.sample_format, AudioSampleFormat::F32 as i32);
}
