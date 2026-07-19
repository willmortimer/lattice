use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use lattice_protocol::{
    encode_frame, envelope, error_envelope, event_envelope, request, response, response_envelope,
    try_decode_frame, Envelope, Event, HealthResponse, PrepareModelResponse, Request, Response,
    UnloadVoiceModelResponse, VoiceHostStatusResponse, PROTOCOL_VERSION,
};
use lattice_voice::{
    InProcessVoiceService, ModelState, ModelStatus, SpeechEventSender, SpeechProvider, VoiceRequest,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use crate::backend::{open_provider, BackendKind};
use crate::convert::{
    capabilities_to_proto, event_from_domain, model_status_to_proto, voice_request_from_proto,
};
use crate::error::VoiceHostError;

/// Host process configuration.
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub socket_path: PathBuf,
    pub backend: BackendKind,
    pub model_cache_dir: Option<PathBuf>,
    pub instance_id: String,
}

impl HostConfig {
    pub fn new(
        socket_path: PathBuf,
        backend: BackendKind,
        model_cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            socket_path,
            backend,
            model_cache_dir,
            instance_id: Uuid::now_v7().to_string(),
        }
    }
}

struct HostInner {
    service: InProcessVoiceService,
    provider: Arc<dyn SpeechProvider>,
    model_status: ModelStatus,
    loaded_model_id: Option<String>,
    active_sessions: u64,
}

/// Shared host runtime state.
pub struct HostState {
    pub config: HostConfig,
    inner: Mutex<HostInner>,
    event_sequence: AtomicU64,
    chunks_accepted: AtomicU64,
    finals_emitted: AtomicU64,
}

impl HostState {
    pub fn new(config: HostConfig) -> Result<Arc<Self>, VoiceHostError> {
        let provider = open_provider(config.backend, config.model_cache_dir.clone())?;
        Ok(Arc::new(Self {
            config,
            inner: Mutex::new(HostInner {
                service: InProcessVoiceService::new(Arc::clone(&provider)),
                provider,
                model_status: ModelStatus {
                    state: ModelState::Unavailable,
                    model_version: None,
                    provider_version: None,
                    message: None,
                },
                loaded_model_id: None,
                active_sessions: 0,
            }),
            event_sequence: AtomicU64::new(1),
            chunks_accepted: AtomicU64::new(0),
            finals_emitted: AtomicU64::new(0),
        }))
    }

    pub fn backend_name(&self) -> &'static str {
        self.config.backend.as_str()
    }

    fn next_event_sequence(&self) -> u64 {
        self.event_sequence.fetch_add(1, Ordering::Relaxed)
    }
}

/// Serve voice-host RPCs on a Unix-domain socket until the listener fails.
pub async fn run_server(state: Arc<HostState>) -> Result<(), VoiceHostError> {
    if let Some(parent) = state.config.socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if state.config.socket_path.exists() {
        tokio::fs::remove_file(&state.config.socket_path).await?;
    }

    let listener = UnixListener::bind(&state.config.socket_path)?;
    info!(
        socket = %state.config.socket_path.display(),
        backend = state.backend_name(),
        "voice-host listening"
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(state, stream).await {
                warn!(error = %error, "voice-host connection closed with error");
            }
        });
    }
}

async fn handle_connection(
    state: Arc<HostState>,
    mut stream: UnixStream,
) -> Result<(), VoiceHostError> {
    let mut buffer = BytesMut::with_capacity(4096);
    let mut read_buf = [0u8; 8192];
    let (events, mut event_rx) = SpeechEventSender::pair();

    loop {
        let n = stream.read(&mut read_buf).await?;
        if n == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&read_buf[..n]);

        while let Some(envelope) = try_decode_frame(&mut buffer)? {
            let (reply, pending_events) =
                dispatch(Arc::clone(&state), envelope, &events, &mut event_rx).await;
            let framed = encode_frame(&reply)?;
            stream.write_all(&framed).await?;
            for event in pending_events {
                let framed = encode_frame(&event_envelope(reply_request_id(&reply), event))?;
                stream.write_all(&framed).await?;
            }
        }
    }
}

fn reply_request_id(envelope: &Envelope) -> String {
    envelope.request_id.clone()
}

async fn dispatch(
    state: Arc<HostState>,
    envelope: Envelope,
    events: &SpeechEventSender,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<lattice_voice::VoiceEvent>,
) -> (Envelope, Vec<Event>) {
    let request_id = envelope.request_id.clone();
    if envelope.protocol_version != PROTOCOL_VERSION {
        return (
            error_envelope(
                request_id,
                lattice_protocol::Error {
                    code: "protocol_version".into(),
                    message: format!(
                        "unsupported protocol version {} (host speaks {})",
                        envelope.protocol_version, PROTOCOL_VERSION
                    ),
                    details: None,
                },
            ),
            Vec::new(),
        );
    }

    let Some(envelope::Payload::Request(request)) = envelope.payload else {
        return (
            error_envelope(
                request_id,
                lattice_protocol::Error {
                    code: "invalid_payload".into(),
                    message: "expected request payload".into(),
                    details: None,
                },
            ),
            Vec::new(),
        );
    };

    match handle_request(Arc::clone(&state), request, events).await {
        Ok(response) => {
            let mut pending = Vec::new();
            while let Ok(event) = event_rx.try_recv() {
                match event_from_domain(state.next_event_sequence(), event) {
                    Ok(wire) => pending.push(wire),
                    Err(error) => {
                        warn!(error = %error, "failed to convert voice event");
                    }
                }
            }
            (response_envelope(request_id, response), pending)
        }
        Err(error) => (error_envelope(request_id, remote_error(error)), Vec::new()),
    }
}

fn remote_error(error: VoiceHostError) -> lattice_protocol::Error {
    let (code, message) = match &error {
        VoiceHostError::ModelNotPrepared => ("model_not_prepared", error.to_string()),
        VoiceHostError::BackendUnavailable(message) => ("backend_unavailable", message.clone()),
        VoiceHostError::Speech(inner) => ("speech", inner.to_string()),
        VoiceHostError::Remote { code, message } => {
            return lattice_protocol::Error {
                code: code.clone(),
                message: message.clone(),
                details: None,
            };
        }
        other => ("host_error", other.to_string()),
    };
    lattice_protocol::Error {
        code: code.into(),
        message,
        details: None,
    }
}

async fn handle_request(
    state: Arc<HostState>,
    request: Request,
    events: &SpeechEventSender,
) -> Result<Response, VoiceHostError> {
    let body = request
        .body
        .ok_or_else(|| VoiceHostError::protocol("request body is required"))?;

    match body {
        request::Body::Health(_) => Ok(Response {
            body: Some(response::Body::Health(HealthResponse {
                status: "ok".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: state.config.instance_id.clone(),
                backend: Some(state.backend_name().into()),
            })),
        }),
        request::Body::VoiceHostStatus(_) => {
            let inner = state.inner.lock().await;
            Ok(Response {
                body: Some(response::Body::VoiceHostStatus(VoiceHostStatusResponse {
                    backend: state.backend_name().into(),
                    model_status: Some(model_status_to_proto(inner.model_status.clone())),
                    active_sessions: inner.active_sessions,
                    chunks_accepted: state.chunks_accepted.load(Ordering::Relaxed),
                    finals_emitted: state.finals_emitted.load(Ordering::Relaxed),
                    loaded_model_id: inner.loaded_model_id.clone(),
                })),
            })
        }
        request::Body::UnloadVoiceModel(_) => handle_unload(state, events).await,
        other => {
            let Some(voice_request) = voice_request_from_proto(other)? else {
                return Err(VoiceHostError::protocol(
                    "request not supported by voice-host (workspace RPCs belong on latticed)",
                ));
            };
            handle_voice_request(state, voice_request, events).await
        }
    }
}

async fn handle_unload(
    state: Arc<HostState>,
    events: &SpeechEventSender,
) -> Result<Response, VoiceHostError> {
    let mut inner = state.inner.lock().await;

    // Drop live sessions by rebuilding the service around a fresh provider.
    let provider = open_provider(
        state.config.backend,
        state.config.model_cache_dir.clone(),
    )?;
    inner.provider = Arc::clone(&provider);
    inner.service = InProcessVoiceService::new(provider);
    inner.active_sessions = 0;
    inner.loaded_model_id = None;
    inner.model_status = ModelStatus {
        state: ModelState::Unavailable,
        model_version: None,
        provider_version: None,
        message: Some("unloaded".into()),
    };
    let status = inner.model_status.clone();
    drop(inner);

    let _ = events.send(lattice_voice::VoiceEvent::ModelStatusChanged(status.clone()));

    Ok(Response {
        body: Some(response::Body::UnloadVoiceModel(UnloadVoiceModelResponse {
            status: Some(model_status_to_proto(status)),
        })),
    })
}

async fn handle_voice_request(
    state: Arc<HostState>,
    voice_request: VoiceRequest,
    events: &SpeechEventSender,
) -> Result<Response, VoiceHostError> {
    let mut inner = state.inner.lock().await;

    match voice_request {
        VoiceRequest::PrepareModel(prepare) => {
            let model_id = prepare.model_id.clone();
            let status = inner
                .provider
                .prepare(prepare)
                .await
                .map_err(VoiceHostError::Speech)?;
            events
                .send(lattice_voice::VoiceEvent::ModelStatusChanged(status.clone()))
                .map_err(VoiceHostError::Speech)?;
            inner.model_status = status.clone();
            if status.state == ModelState::Ready {
                inner.loaded_model_id = Some(model_id);
            }
            Ok(Response {
                body: Some(response::Body::PrepareModel(PrepareModelResponse {
                    status: Some(model_status_to_proto(status)),
                })),
            })
        }
        VoiceRequest::GetVoiceCapabilities => {
            let capabilities = inner.provider.capabilities();
            Ok(Response {
                body: Some(response::Body::GetVoiceCapabilities(
                    lattice_protocol::GetVoiceCapabilitiesResponse {
                        capabilities: Some(capabilities_to_proto(capabilities)),
                    },
                )),
            })
        }
        VoiceRequest::StartVoiceSession(start) => {
            let session_id = start.config.session_id.clone();
            inner
                .service
                .handle_request(VoiceRequest::StartVoiceSession(start), events)
                .await?;
            inner.active_sessions = inner.active_sessions.saturating_add(1);
            let capabilities = inner.provider.capabilities();
            Ok(Response {
                body: Some(response::Body::StartVoiceSession(
                    lattice_protocol::StartVoiceSessionResponse {
                        session_id,
                        protocol_version: PROTOCOL_VERSION,
                        capabilities: Some(capabilities_to_proto(capabilities)),
                    },
                )),
            })
        }
        VoiceRequest::PushAudioChunk(chunk) => {
            let sequence = chunk.sequence;
            inner
                .service
                .handle_request(VoiceRequest::PushAudioChunk(chunk), events)
                .await?;
            state.chunks_accepted.fetch_add(1, Ordering::Relaxed);
            Ok(Response {
                body: Some(response::Body::PushAudioChunk(
                    lattice_protocol::PushAudioChunkResponse { sequence },
                )),
            })
        }
        VoiceRequest::FinishUtterance(finish) => {
            inner
                .service
                .handle_request(VoiceRequest::FinishUtterance(finish), events)
                .await?;
            state.finals_emitted.fetch_add(1, Ordering::Relaxed);
            inner.active_sessions = inner.active_sessions.saturating_sub(1);
            Ok(Response {
                body: Some(response::Body::FinishUtterance(
                    lattice_protocol::FinishUtteranceResponse {},
                )),
            })
        }
        VoiceRequest::UpdateSessionContext(update) => {
            inner
                .service
                .handle_request(VoiceRequest::UpdateSessionContext(update), events)
                .await?;
            Ok(Response {
                body: Some(response::Body::UpdateSessionContext(
                    lattice_protocol::UpdateSessionContextResponse {},
                )),
            })
        }
        VoiceRequest::CancelVoiceSession(cancel) => {
            inner
                .service
                .handle_request(VoiceRequest::CancelVoiceSession(cancel), events)
                .await?;
            inner.active_sessions = inner.active_sessions.saturating_sub(1);
            Ok(Response {
                body: Some(response::Body::CancelVoiceSession(
                    lattice_protocol::CancelVoiceSessionResponse {},
                )),
            })
        }
        VoiceRequest::EndVoiceSession(end) => {
            inner
                .service
                .handle_request(VoiceRequest::EndVoiceSession(end), events)
                .await?;
            inner.active_sessions = inner.active_sessions.saturating_sub(1);
            Ok(Response {
                body: Some(response::Body::EndVoiceSession(
                    lattice_protocol::EndVoiceSessionResponse {},
                )),
            })
        }
    }
}
