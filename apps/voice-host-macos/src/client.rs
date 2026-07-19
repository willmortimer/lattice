use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use lattice_protocol::{
    encode_frame, envelope, request, request_envelope, response, Event,
    FinishUtteranceRequest, FrameDecoder, GetVoiceCapabilitiesRequest, HealthRequest,
    HealthResponse, PrepareModelRequest, PushAudioChunkRequest, Request, Response,
    SessionContext, SpeechSessionConfig, StartVoiceSessionRequest, UnloadVoiceModelRequest,
    UnloadVoiceModelResponse, VoiceHostStatusRequest, VoiceHostStatusResponse, PROTOCOL_VERSION,
};
use lattice_voice::{
    AudioChunk, AudioSampleFormat, ModelStatus, SpeechCapabilities, PROTOCOL_VERSION as VOICE_PROTOCOL_VERSION,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, oneshot, Mutex};
use uuid::Uuid;

use crate::convert::{capabilities_from_proto, model_status_from_proto};
use crate::error::VoiceHostError;

/// Client that speaks the voice-host UDS protocol (lattice-protocol envelopes).
pub struct VoiceHostClient {
    socket_path: PathBuf,
    writer: Mutex<OwnedWriteHalf>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, VoiceHostError>>>>>,
    event_tx: broadcast::Sender<Event>,
    next_request_id: AtomicU64,
}

impl VoiceHostClient {
    /// Connect to a running voice-host socket.
    pub async fn connect(socket_path: impl AsRef<Path>) -> Result<Self, VoiceHostError> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let stream = UnixStream::connect(&socket_path).await?;
        let (reader, writer) = stream.into_split();
        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, VoiceHostError>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(64);
        spawn_reader(reader, Arc::clone(&pending), event_tx.clone());

        Ok(Self {
            socket_path,
            writer: Mutex::new(writer),
            pending,
            event_tx,
            next_request_id: AtomicU64::new(1),
        })
    }

    /// Socket path used for this connection.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Subscribe to push events (partials, finals, model status, …).
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Health check.
    pub async fn health(&self) -> Result<HealthResponse, VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::Health(HealthRequest {})),
            })
            .await?;
        match response.body {
            Some(response::Body::Health(health)) => Ok(health),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected health response: {other:?}"
            ))),
        }
    }

    /// Host status and metrics.
    pub async fn status(&self) -> Result<VoiceHostStatusResponse, VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::VoiceHostStatus(VoiceHostStatusRequest {})),
            })
            .await?;
        match response.body {
            Some(response::Body::VoiceHostStatus(status)) => Ok(status),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected status response: {other:?}"
            ))),
        }
    }

    /// Prepare / warm a speech model.
    pub async fn prepare_model(
        &self,
        model_id: impl Into<String>,
        warm: bool,
    ) -> Result<ModelStatus, VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::PrepareModel(PrepareModelRequest {
                    model_id: model_id.into(),
                    warm,
                })),
            })
            .await?;
        match response.body {
            Some(response::Body::PrepareModel(prepare)) => {
                let status = prepare
                    .status
                    .ok_or_else(|| VoiceHostError::protocol("prepare response missing status"))?;
                model_status_from_proto(status)
            }
            other => Err(VoiceHostError::protocol(format!(
                "unexpected prepare response: {other:?}"
            ))),
        }
    }

    /// Unload the current model and drop active sessions.
    pub async fn unload_model(&self) -> Result<UnloadVoiceModelResponse, VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::UnloadVoiceModel(UnloadVoiceModelRequest {})),
            })
            .await?;
        match response.body {
            Some(response::Body::UnloadVoiceModel(unload)) => Ok(unload),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected unload response: {other:?}"
            ))),
        }
    }

    /// Negotiate provider capabilities.
    pub async fn capabilities(&self) -> Result<SpeechCapabilities, VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::GetVoiceCapabilities(
                    GetVoiceCapabilitiesRequest {},
                )),
            })
            .await?;
        match response.body {
            Some(response::Body::GetVoiceCapabilities(caps)) => {
                let capabilities = caps.capabilities.ok_or_else(|| {
                    VoiceHostError::protocol("capabilities response missing body")
                })?;
                capabilities_from_proto(capabilities)
            }
            other => Err(VoiceHostError::protocol(format!(
                "unexpected capabilities response: {other:?}"
            ))),
        }
    }

    /// Start a voice session.
    pub async fn start_session(
        &self,
        session_id: impl Into<String>,
        language: Option<String>,
    ) -> Result<String, VoiceHostError> {
        let session_id = session_id.into();
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::StartVoiceSession(StartVoiceSessionRequest {
                    config: Some(SpeechSessionConfig {
                        session_id: session_id.clone(),
                        language,
                        context: Some(SessionContext {
                            document_id: None,
                            glossary_terms: Vec::new(),
                            command_mode: false,
                        }),
                    }),
                })),
            })
            .await?;
        match response.body {
            Some(response::Body::StartVoiceSession(started)) => {
                if started.protocol_version != VOICE_PROTOCOL_VERSION
                    && started.protocol_version != PROTOCOL_VERSION
                {
                    return Err(VoiceHostError::protocol(format!(
                        "unexpected session protocol version {}",
                        started.protocol_version
                    )));
                }
                Ok(started.session_id)
            }
            other => Err(VoiceHostError::protocol(format!(
                "unexpected start_session response: {other:?}"
            ))),
        }
    }

    /// Push one packed PCM chunk.
    pub async fn push_audio(&self, chunk: AudioChunk) -> Result<u64, VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::PushAudioChunk(PushAudioChunkRequest {
                    session_id: chunk.session_id,
                    sequence: chunk.sequence,
                    captured_at_ns: chunk.captured_at_ns,
                    sample_rate_hz: chunk.sample_rate_hz,
                    channels: u32::from(chunk.channels),
                    sample_format: sample_format_to_proto(chunk.sample_format).into(),
                    payload: chunk.payload.to_vec(),
                })),
            })
            .await?;
        match response.body {
            Some(response::Body::PushAudioChunk(pushed)) => Ok(pushed.sequence),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected push_audio response: {other:?}"
            ))),
        }
    }

    /// Finish the current utterance and wait for finalization side-effects.
    pub async fn finish_utterance(
        &self,
        session_id: impl Into<String>,
        utterance_id: impl Into<String>,
    ) -> Result<(), VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::FinishUtterance(FinishUtteranceRequest {
                    session_id: session_id.into(),
                    utterance_id: utterance_id.into(),
                })),
            })
            .await?;
        match response.body {
            Some(response::Body::FinishUtterance(_)) => Ok(()),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected finish_utterance response: {other:?}"
            ))),
        }
    }

    /// Convenience: push a fixture PCM buffer as a single chunk (sequence 0).
    pub async fn push_fixture_pcm(
        &self,
        session_id: impl Into<String>,
        pcm: impl Into<Bytes>,
    ) -> Result<u64, VoiceHostError> {
        self.push_audio(AudioChunk {
            session_id: session_id.into(),
            sequence: 0,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: pcm.into(),
        })
        .await
    }

    /// Update session glossary / document context.
    pub async fn update_session_context(
        &self,
        session_id: impl Into<String>,
        context: SessionContext,
    ) -> Result<(), VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::UpdateSessionContext(
                    lattice_protocol::UpdateSessionContextRequest {
                        session_id: session_id.into(),
                        context: Some(context),
                    },
                )),
            })
            .await?;
        match response.body {
            Some(response::Body::UpdateSessionContext(_)) => Ok(()),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected update_session_context response: {other:?}"
            ))),
        }
    }

    /// Cancel an active voice session.
    pub async fn cancel_session(
        &self,
        session_id: impl Into<String>,
        reason: Option<String>,
    ) -> Result<(), VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::CancelVoiceSession(
                    lattice_protocol::CancelVoiceSessionRequest {
                        session_id: session_id.into(),
                        reason,
                    },
                )),
            })
            .await?;
        match response.body {
            Some(response::Body::CancelVoiceSession(_)) => Ok(()),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected cancel_session response: {other:?}"
            ))),
        }
    }

    /// End an active voice session cleanly.
    pub async fn end_session(&self, session_id: impl Into<String>) -> Result<(), VoiceHostError> {
        let response = self
            .call(Request {
                deadline_unix_ms: None,
                idempotency_key: None,
                body: Some(request::Body::EndVoiceSession(
                    lattice_protocol::EndVoiceSessionRequest {
                        session_id: session_id.into(),
                    },
                )),
            })
            .await?;
        match response.body {
            Some(response::Body::EndVoiceSession(_)) => Ok(()),
            other => Err(VoiceHostError::protocol(format!(
                "unexpected end_session response: {other:?}"
            ))),
        }
    }

    /// Forward an arbitrary request envelope body to the host (daemon proxy).
    pub async fn forward(&self, request: Request) -> Result<Response, VoiceHostError> {
        self.call(request).await
    }

    async fn call(&self, request: Request) -> Result<Response, VoiceHostError> {
        let request_id = self.alloc_request_id();
        let envelope = request_envelope(request_id.clone(), request);
        let framed = encode_frame(&envelope)?;

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id, tx);
        }

        {
            let mut writer = self.writer.lock().await;
            writer.write_all(&framed).await?;
            writer.flush().await?;
        }

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(VoiceHostError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "voice-host response channel closed",
            ))),
        }
    }

    fn alloc_request_id(&self) -> String {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        format!("vh-{id}-{}", Uuid::now_v7())
    }
}

fn sample_format_to_proto(
    format: AudioSampleFormat,
) -> lattice_protocol::AudioSampleFormat {
    match format {
        AudioSampleFormat::F32 => lattice_protocol::AudioSampleFormat::F32,
        AudioSampleFormat::I16Le => lattice_protocol::AudioSampleFormat::I16Le,
    }
}

fn spawn_reader(
    mut reader: OwnedReadHalf,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Response, VoiceHostError>>>>>,
    event_tx: broadcast::Sender<Event>,
) {
    tokio::spawn(async move {
        let mut read_buf = BytesMut::new();
        let mut decoder = FrameDecoder::new();
        loop {
            let envelope = match read_envelope(&mut reader, &mut read_buf, &mut decoder).await {
                Ok(envelope) => envelope,
                Err(_) => {
                    let mut guard = pending.lock().await;
                    for (_, tx) in guard.drain() {
                        let _ = tx.send(Err(VoiceHostError::Io(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "voice-host connection closed",
                        ))));
                    }
                    break;
                }
            };

            match envelope.payload {
                Some(envelope::Payload::Response(response)) => {
                    let mut guard = pending.lock().await;
                    if let Some(tx) = guard.remove(&envelope.request_id) {
                        let _ = tx.send(Ok(response));
                    }
                }
                Some(envelope::Payload::Error(error)) => {
                    let mut guard = pending.lock().await;
                    if let Some(tx) = guard.remove(&envelope.request_id) {
                        let _ = tx.send(Err(VoiceHostError::remote(error.code, error.message)));
                    }
                }
                Some(envelope::Payload::Event(event)) => {
                    let _ = event_tx.send(event);
                }
                Some(envelope::Payload::Request(_)) | None => {}
            }
        }
    });
}

async fn read_envelope(
    reader: &mut OwnedReadHalf,
    read_buf: &mut BytesMut,
    decoder: &mut FrameDecoder,
) -> Result<lattice_protocol::Envelope, VoiceHostError> {
    loop {
        if let Some(envelope) = decoder.decode(read_buf)? {
            return Ok(envelope);
        }
        let mut tmp = [0u8; 8192];
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            return Err(VoiceHostError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "voice-host closed connection while waiting for envelope",
            )));
        }
        read_buf.extend_from_slice(&tmp[..n]);
    }
}

/// Helper used by tests to build a socket path under a temp directory.
pub fn socket_path_in(dir: impl AsRef<Path>) -> PathBuf {
    dir.as_ref().join("voice-host.sock")
}

/// Drain events until a final transcript is observed (or the channel closes).
pub async fn wait_for_final_transcript(
    events: &mut broadcast::Receiver<Event>,
) -> Result<String, VoiceHostError> {
    loop {
        match events.recv().await {
            Ok(event) => {
                if let Some(lattice_protocol::event::Body::FinalTranscript(final_transcript)) =
                    event.body
                {
                    return Ok(final_transcript.text);
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                return Err(VoiceHostError::protocol(
                    "event channel closed before final transcript",
                ));
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
}

/// Collect partial + final transcript texts for a short fixture session.
pub async fn collect_transcript_texts(
    events: &mut broadcast::Receiver<Event>,
    expected_partials: usize,
) -> Result<(Vec<String>, Option<String>), VoiceHostError> {
    let mut partials = Vec::new();
    let mut final_text = None;
    while partials.len() < expected_partials || final_text.is_none() {
        match events.recv().await {
            Ok(event) => match event.body {
                Some(lattice_protocol::event::Body::PartialTranscript(partial)) => {
                    partials.push(partial.text);
                }
                Some(lattice_protocol::event::Body::FinalTranscript(final_transcript)) => {
                    final_text = Some(final_transcript.text);
                }
                _ => {}
            },
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
    Ok((partials, final_text))
}
