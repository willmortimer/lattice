use lattice_protocol::{
    decode_frame, encode_frame, event_envelope, request_envelope, response_envelope,
    ApplyPageUpdateRequest, ApplyPageUpdateResponse, AudioGap, AudioSampleFormat,
    CancelVoiceSessionRequest, CancelVoiceSessionResponse, Event, FinalTranscript,
    FinalizationMode, FinishUtteranceRequest, FinishUtteranceResponse, HealthRequest,
    HealthResponse, IndexProgress, ModelState, ModelStatus, ModelStatusChanged,
    OpenWorkspaceRequest, OpenWorkspaceResponse, PartialTranscript, PingRequest,
    PrepareModelRequest, PrepareModelResponse, PushAudioChunkRequest, PushAudioChunkResponse,
    Request, Response, ResourceChanged, SearchRequest, SearchResponse, SessionContext,
    SessionFailed, SpeechCapabilities, SpeechSessionConfig, StartVoiceSessionRequest,
    StartVoiceSessionResponse, TranscriptionSessionState, UpdateSessionContextRequest,
    UpdateSessionContextResponse, WorkspaceLease, WorkspaceLeaseChanged, PROTOCOL_VERSION,
};
use prost::Message;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn load_hex_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    hex::decode(&cleaned).unwrap_or_else(|err| panic!("decode {}: {err}", path.display()))
}

fn health_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-health-req",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::Health(HealthRequest {})),
        },
    )
}

fn health_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-health-res",
        Response {
            body: Some(lattice_protocol::response::Body::Health(HealthResponse {
                status: "ok".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: "0190abcdef0123456789".into(),
                backend: None,
            })),
        },
    )
}

fn ping_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-ping",
        Request {
            deadline_unix_ms: Some(1_720_000_000_000),
            idempotency_key: Some("idem-golden-ping".into()),
            body: Some(lattice_protocol::request::Body::Ping(PingRequest {
                nonce: "n-42".into(),
            })),
        },
    )
}

fn open_workspace_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-open",
        Request {
            deadline_unix_ms: None,
            idempotency_key: Some("idem-open-ws".into()),
            body: Some(lattice_protocol::request::Body::OpenWorkspace(
                OpenWorkspaceRequest {
                    path: "/tmp/example-workspace".into(),
                },
            )),
        },
    )
}

fn open_workspace_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-open",
        Response {
            body: Some(lattice_protocol::response::Body::OpenWorkspace(
                OpenWorkspaceResponse {
                    workspace_id: "ws-1".into(),
                    lease: Some(WorkspaceLease {
                        schema_version: 1,
                        owner: "latticed".into(),
                        pid: 12345,
                        process_start: 987_654_321,
                        socket: "/tmp/latticed.sock".into(),
                        protocol_version: PROTOCOL_VERSION,
                        instance_id: "0190instance".into(),
                        acquired_at: "2026-07-19T20:00:00Z".into(),
                    }),
                },
            )),
        },
    )
}

fn search_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-search",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::Search(SearchRequest {
                workspace_id: "ws-1".into(),
                query: "hello lattice".into(),
            })),
        },
    )
}

fn search_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-search",
        Response {
            body: Some(lattice_protocol::response::Body::Search(SearchResponse {})),
        },
    )
}

fn apply_page_update_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-apply",
        Request {
            deadline_unix_ms: None,
            idempotency_key: Some("idem-apply-1".into()),
            body: Some(lattice_protocol::request::Body::ApplyPageUpdate(
                ApplyPageUpdateRequest {
                    workspace_id: "ws-1".into(),
                    path: "Notes.md".into(),
                    content: "# Updated\n".into(),
                    expected_revision: "sha256:abc".into(),
                },
            )),
        },
    )
}

fn apply_page_update_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-apply",
        Response {
            body: Some(lattice_protocol::response::Body::ApplyPageUpdate(
                ApplyPageUpdateResponse {
                    revision: "sha256:def".into(),
                },
            )),
        },
    )
}

fn lease_changed_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 7,
            workspace_id: "ws-1".into(),
            body: Some(lattice_protocol::event::Body::LeaseChanged(
                WorkspaceLeaseChanged {
                    lease: Some(WorkspaceLease {
                        schema_version: 1,
                        owner: "embedded".into(),
                        pid: 99,
                        process_start: 1,
                        socket: String::new(),
                        protocol_version: PROTOCOL_VERSION,
                        instance_id: "embedded-1".into(),
                        acquired_at: "2026-07-19T21:00:00Z".into(),
                    }),
                },
            )),
        },
    )
}

fn resource_changed_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 8,
            workspace_id: "ws-1".into(),
            body: Some(lattice_protocol::event::Body::ResourceChanged(
                ResourceChanged {
                    path: "Notes.md".into(),
                    change: "modified".into(),
                    revision: Some("sha256:abc".into()),
                    from_path: None,
                },
            )),
        },
    )
}

fn index_progress_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 9,
            workspace_id: "ws-1".into(),
            body: Some(lattice_protocol::event::Body::IndexProgress(IndexProgress {
                phase: "upserted".into(),
                path: Some("Notes.md".into()),
                detail: None,
            })),
        },
    )
}

fn sample_capabilities() -> SpeechCapabilities {
    SpeechCapabilities {
        streaming: true,
        partial_transcripts: true,
        finalization_mode: FinalizationMode::StreamingFlush as i32,
        punctuation: true,
        word_timestamps: false,
        language_detection: false,
        vocabulary_biasing: true,
        endpoint_detection: true,
        supported_languages: vec!["en".into()],
    }
}

fn prepare_model_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-prepare-model",
        Request {
            deadline_unix_ms: None,
            idempotency_key: Some("idem-prepare-1".into()),
            body: Some(lattice_protocol::request::Body::PrepareModel(
                PrepareModelRequest {
                    model_id: "parakeet-tdt-0.6b".into(),
                    warm: true,
                },
            )),
        },
    )
}

fn prepare_model_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-prepare-model",
        Response {
            body: Some(lattice_protocol::response::Body::PrepareModel(
                PrepareModelResponse {
                    status: Some(ModelStatus {
                        state: ModelState::Ready as i32,
                        model_version: Some("1.0.0".into()),
                        provider_version: Some("fluidaudio-0.1".into()),
                        message: None,
                    }),
                },
            )),
        },
    )
}

fn start_voice_session_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-start-voice",
        Request {
            deadline_unix_ms: Some(1_720_000_000_100),
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::StartVoiceSession(
                StartVoiceSessionRequest {
                    config: Some(SpeechSessionConfig {
                        session_id: "vs-1".into(),
                        language: Some("en".into()),
                        context: Some(SessionContext {
                            document_id: Some("Notes.md".into()),
                            glossary_terms: vec!["Lattice".into(), "latticed".into()],
                            command_mode: false,
                            known_paths: vec![],
                        }),
                        endpoint: None,
                    }),
                },
            )),
        },
    )
}

fn start_voice_session_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-start-voice",
        Response {
            body: Some(lattice_protocol::response::Body::StartVoiceSession(
                StartVoiceSessionResponse {
                    session_id: "vs-1".into(),
                    protocol_version: PROTOCOL_VERSION,
                    capabilities: Some(sample_capabilities()),
                },
            )),
        },
    )
}

fn push_audio_chunk_request() -> lattice_protocol::Envelope {
    // Four Float32 LE samples: 0.0, 0.5, -0.5, 1.0
    let payload: Vec<u8> = [
        0.0f32.to_le_bytes(),
        0.5f32.to_le_bytes(),
        (-0.5f32).to_le_bytes(),
        1.0f32.to_le_bytes(),
    ]
    .into_iter()
    .flatten()
    .collect();
    request_envelope(
        "golden-push-audio",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::PushAudioChunk(
                PushAudioChunkRequest {
                    session_id: "vs-1".into(),
                    sequence: 42,
                    captured_at_ns: 1_720_000_000_000_000_000,
                    sample_rate_hz: 16_000,
                    channels: 1,
                    sample_format: AudioSampleFormat::F32 as i32,
                    payload,
                },
            )),
        },
    )
}

fn push_audio_chunk_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-push-audio",
        Response {
            body: Some(lattice_protocol::response::Body::PushAudioChunk(
                PushAudioChunkResponse { sequence: 42 },
            )),
        },
    )
}

fn finish_utterance_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-finish",
        Request {
            deadline_unix_ms: None,
            idempotency_key: Some("idem-finish-1".into()),
            body: Some(lattice_protocol::request::Body::FinishUtterance(
                FinishUtteranceRequest {
                    session_id: "vs-1".into(),
                    utterance_id: "utt-9".into(),
                },
            )),
        },
    )
}

fn finish_utterance_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-finish",
        Response {
            body: Some(lattice_protocol::response::Body::FinishUtterance(
                FinishUtteranceResponse {},
            )),
        },
    )
}

fn update_session_context_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-update-ctx",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::UpdateSessionContext(
                UpdateSessionContextRequest {
                    session_id: "vs-1".into(),
                    context: Some(SessionContext {
                        document_id: Some("Inbox.md".into()),
                        glossary_terms: vec!["Quick Note".into()],
                        command_mode: true,
                        known_paths: vec![],
                    }),
                },
            )),
        },
    )
}

fn update_session_context_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-update-ctx",
        Response {
            body: Some(lattice_protocol::response::Body::UpdateSessionContext(
                UpdateSessionContextResponse {},
            )),
        },
    )
}

fn cancel_voice_session_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-cancel-voice",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::CancelVoiceSession(
                CancelVoiceSessionRequest {
                    session_id: "vs-1".into(),
                    reason: Some("user aborted".into()),
                },
            )),
        },
    )
}

fn cancel_voice_session_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-cancel-voice",
        Response {
            body: Some(lattice_protocol::response::Body::CancelVoiceSession(
                CancelVoiceSessionResponse {},
            )),
        },
    )
}

fn model_status_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 20,
            workspace_id: String::new(),
            body: Some(lattice_protocol::event::Body::ModelStatus(
                ModelStatusChanged {
                    status: Some(ModelStatus {
                        state: ModelState::Preparing as i32,
                        model_version: Some("1.0.0".into()),
                        provider_version: None,
                        message: Some("warming".into()),
                    }),
                },
            )),
        },
    )
}

fn partial_transcript_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 21,
            workspace_id: String::new(),
            body: Some(lattice_protocol::event::Body::PartialTranscript(
                PartialTranscript {
                    session_id: "vs-1".into(),
                    utterance_id: "utt-9".into(),
                    revision: 3,
                    text: "hello lat".into(),
                    stable_prefix_bytes: 5,
                    started_at_ms: 100,
                    ended_at_ms: 250,
                },
            )),
        },
    )
}

fn final_transcript_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 22,
            workspace_id: String::new(),
            body: Some(lattice_protocol::event::Body::FinalTranscript(
                FinalTranscript {
                    session_id: "vs-1".into(),
                    utterance_id: "utt-9".into(),
                    replaces_revision: 3,
                    text: "hello lattice".into(),
                    finalization_mode: FinalizationMode::StreamingFlush as i32,
                    duration_ms: 1_200,
                    processing_ms: 40,
                },
            )),
        },
    )
}

fn session_failed_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 23,
            workspace_id: String::new(),
            body: Some(lattice_protocol::event::Body::SessionFailed(SessionFailed {
                session_id: "vs-1".into(),
                message: "host crashed".into(),
                state: TranscriptionSessionState::Failed as i32,
            })),
        },
    )
}

fn audio_gap_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 24,
            workspace_id: String::new(),
            body: Some(lattice_protocol::event::Body::AudioGap(AudioGap {
                session_id: "vs-1".into(),
                last_contiguous_sequence: 10,
                next_sequence: 15,
                detected_at_ns: 1_720_000_000_000_123_456,
                reason: Some("queue overflow".into()),
            })),
        },
    )
}

fn assert_golden(name: &str, envelope: &lattice_protocol::Envelope) {
    let expected = load_hex_fixture(name);
    let actual = envelope.encode_to_vec();
    assert_eq!(
        hex::encode(&actual),
        hex::encode(&expected),
        "protobuf bytes for {name} drifted from golden fixture"
    );
    let decoded = lattice_protocol::Envelope::decode(expected.as_slice()).expect("decode golden");
    assert_eq!(&decoded, envelope);

    let framed = encode_frame(envelope).expect("frame encode");
    let round_trip = decode_frame(&framed).expect("frame decode");
    assert_eq!(&round_trip, envelope);
}

#[test]
fn golden_health_request() {
    assert_golden("health_request.hex", &health_request());
}

#[test]
fn golden_health_response() {
    assert_golden("health_response.hex", &health_response());
}

#[test]
fn golden_ping_request() {
    assert_golden("ping_request.hex", &ping_request());
}

#[test]
fn golden_open_workspace_request() {
    assert_golden("open_workspace_request.hex", &open_workspace_request());
}

#[test]
fn golden_open_workspace_response() {
    assert_golden("open_workspace_response.hex", &open_workspace_response());
}

#[test]
fn golden_search_request() {
    assert_golden("search_request.hex", &search_request());
}

#[test]
fn golden_search_response() {
    assert_golden("search_response.hex", &search_response());
}

#[test]
fn golden_apply_page_update_request() {
    assert_golden(
        "apply_page_update_request.hex",
        &apply_page_update_request(),
    );
}

#[test]
fn golden_apply_page_update_response() {
    assert_golden(
        "apply_page_update_response.hex",
        &apply_page_update_response(),
    );
}

#[test]
fn golden_lease_changed_event() {
    assert_golden("lease_changed_event.hex", &lease_changed_event());
}

#[test]
fn golden_resource_changed_event() {
    assert_golden("resource_changed_event.hex", &resource_changed_event());
}

#[test]
fn golden_index_progress_event() {
    assert_golden("index_progress_event.hex", &index_progress_event());
}

#[test]
fn golden_prepare_model_request() {
    assert_golden("prepare_model_request.hex", &prepare_model_request());
}

#[test]
fn golden_prepare_model_response() {
    assert_golden("prepare_model_response.hex", &prepare_model_response());
}

#[test]
fn golden_start_voice_session_request() {
    assert_golden(
        "start_voice_session_request.hex",
        &start_voice_session_request(),
    );
}

#[test]
fn golden_start_voice_session_response() {
    assert_golden(
        "start_voice_session_response.hex",
        &start_voice_session_response(),
    );
}

#[test]
fn golden_push_audio_chunk_request() {
    assert_golden("push_audio_chunk_request.hex", &push_audio_chunk_request());
}

#[test]
fn golden_push_audio_chunk_response() {
    assert_golden(
        "push_audio_chunk_response.hex",
        &push_audio_chunk_response(),
    );
}

#[test]
fn golden_finish_utterance_request() {
    assert_golden("finish_utterance_request.hex", &finish_utterance_request());
}

#[test]
fn golden_finish_utterance_response() {
    assert_golden(
        "finish_utterance_response.hex",
        &finish_utterance_response(),
    );
}

#[test]
fn golden_update_session_context_request() {
    assert_golden(
        "update_session_context_request.hex",
        &update_session_context_request(),
    );
}

#[test]
fn golden_update_session_context_response() {
    assert_golden(
        "update_session_context_response.hex",
        &update_session_context_response(),
    );
}

#[test]
fn golden_cancel_voice_session_request() {
    assert_golden(
        "cancel_voice_session_request.hex",
        &cancel_voice_session_request(),
    );
}

#[test]
fn golden_cancel_voice_session_response() {
    assert_golden(
        "cancel_voice_session_response.hex",
        &cancel_voice_session_response(),
    );
}

#[test]
fn golden_model_status_event() {
    assert_golden("model_status_event.hex", &model_status_event());
}

#[test]
fn golden_partial_transcript_event() {
    assert_golden("partial_transcript_event.hex", &partial_transcript_event());
}

#[test]
fn golden_final_transcript_event() {
    assert_golden("final_transcript_event.hex", &final_transcript_event());
}

#[test]
fn golden_session_failed_event() {
    assert_golden("session_failed_event.hex", &session_failed_event());
}

#[test]
fn golden_audio_gap_event() {
    assert_golden("audio_gap_event.hex", &audio_gap_event());
}

#[test]
fn push_audio_chunk_payload_is_packed_f32_bytes() {
    let envelope = push_audio_chunk_request();
    let Some(lattice_protocol::envelope::Payload::Request(req)) = envelope.payload else {
        panic!("expected request");
    };
    let Some(lattice_protocol::request::Body::PushAudioChunk(chunk)) = req.body else {
        panic!("expected push audio chunk");
    };
    assert_eq!(chunk.payload.len(), 16);
    assert_eq!(chunk.sequence, 42);
    assert_eq!(chunk.captured_at_ns, 1_720_000_000_000_000_000);
    assert_eq!(chunk.sample_format, AudioSampleFormat::F32 as i32);
    let samples: Vec<f32> = chunk
        .payload
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    assert_eq!(samples, vec![0.0, 0.5, -0.5, 1.0]);
}

#[test]
fn protocol_version_constant_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(health_response().protocol_version, 1);
}
