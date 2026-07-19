//! `FluidAudioSpeechProvider` — macOS `SpeechProvider` over the C ABI.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lattice_voice::{
    AudioChunk, AudioSampleFormat, DecodeMode, FinalTranscript, ModelState, ModelStatus,
    PartialTranscriptPayload, PrepareModelRequest, SpeechCapabilities, SpeechError, SpeechEventSender,
    SpeechProvider, SpeechSession, SpeechSessionConfig, StableTranscriptPayload, VoiceEvent,
};
use tokio::task::JoinHandle;

use crate::bridge::{
    bridge_event_callback, new_backend, CallbackContext, CallbackContextPtr, OwnedBridgeEvent,
    VoiceBridgeBackend,
};
use crate::error::ensure_abi_version;
use crate::ffi::{LatticeVoiceEngine, LatticeVoiceEventKind, LatticeVoiceSession};
use crate::LATTICE_VOICE_BRIDGE_ABI_VERSION;

const DEFAULT_MODEL_ID: &str = "parakeet-unified-320ms";
const PROVIDER_VERSION: &str = "fluidaudio-unified-0.15.5";

/// Shared session state updated by the callback dispatcher.
#[derive(Debug, Default)]
struct SessionSharedState {
    revision: u64,
    final_text: Option<String>,
}

/// macOS FluidAudio Unified provider (Parakeet 320 ms streaming tier).
pub struct FluidAudioSpeechProvider {
    backend: Arc<dyn VoiceBridgeBackend>,
    model_cache_dir: PathBuf,
    engine: Mutex<Option<LatticeVoiceEngine>>,
    prepared: AtomicBool,
}

impl FluidAudioSpeechProvider {
    /// Create a provider, verifying the native ABI when linked.
    pub fn new() -> Result<Self, SpeechError> {
        let backend = new_backend()?;
        Ok(Self::with_backend(backend, default_model_cache_dir()))
    }

    /// Construct with an explicit model cache directory.
    pub fn with_model_cache(model_cache_dir: impl Into<PathBuf>) -> Result<Self, SpeechError> {
        let backend = new_backend()?;
        Ok(Self::with_backend(backend, model_cache_dir.into()))
    }

    pub(crate) fn with_backend(
        backend: Arc<dyn VoiceBridgeBackend>,
        model_cache_dir: PathBuf,
    ) -> Self {
        Self {
            backend,
            model_cache_dir,
            engine: Mutex::new(None),
            prepared: AtomicBool::new(false),
        }
    }

    fn capabilities_inner() -> SpeechCapabilities {
        SpeechCapabilities {
            streaming: true,
            partial_transcripts: true,
            offline_final_decode: true,
            punctuation: false,
            word_timestamps: false,
            language_detection: false,
            vocabulary_biasing: false,
            endpoint_detection: false,
            supported_languages: vec!["en".into()],
        }
    }

    fn ensure_engine(&self) -> Result<LatticeVoiceEngine, SpeechError> {
        let mut guard = self
            .engine
            .lock()
            .map_err(|_| SpeechError::provider("engine lock poisoned"))?;

        if let Some(engine) = *guard {
            return Ok(engine);
        }

        let cache = if self.model_cache_dir.as_os_str().is_empty() {
            None
        } else {
            Some(self.model_cache_dir.as_path())
        };

        let engine = self.backend.engine_create(cache)?;
        *guard = Some(engine);
        Ok(engine)
    }
}

impl Drop for FluidAudioSpeechProvider {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.engine.lock() {
            if let Some(engine) = guard.take() {
                self.backend.engine_destroy(engine);
            }
        }
    }
}

#[async_trait]
impl SpeechProvider for FluidAudioSpeechProvider {
    fn capabilities(&self) -> SpeechCapabilities {
        Self::capabilities_inner()
    }

    async fn prepare(&self, request: PrepareModelRequest) -> Result<ModelStatus, SpeechError> {
        if request.model_id != DEFAULT_MODEL_ID && !request.model_id.is_empty() {
            return Err(SpeechError::provider(format!(
                "unsupported model_id `{}` (expected `{DEFAULT_MODEL_ID}`)",
                request.model_id
            )));
        }

        ensure_abi_version(
            LATTICE_VOICE_BRIDGE_ABI_VERSION,
            self.backend.abi_version(),
        )?;

        let backend = Arc::clone(&self.backend);
        let engine = self.ensure_engine()?;

        tokio::task::spawn_blocking(move || backend.engine_prepare(engine))
            .await
            .map_err(|err| SpeechError::provider(format!("prepare task failed: {err}")))??;

        self.prepared.store(true, Ordering::Release);

        Ok(ModelStatus {
            state: ModelState::Ready,
            model_version: Some("parakeet-unified-en-0.6b-coreml".into()),
            provider_version: Some(PROVIDER_VERSION.into()),
            message: None,
        })
    }

    async fn start_session(
        &self,
        config: SpeechSessionConfig,
        events: SpeechEventSender,
    ) -> Result<Box<dyn SpeechSession>, SpeechError> {
        if !self.prepared.load(Ordering::Acquire) {
            return Err(SpeechError::provider("engine is not prepared"));
        }

        let engine = self.ensure_engine()?;
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let callback_ctx = Box::new(CallbackContext {
            tx: event_tx,
            cancelled: cancelled.clone(),
        });
        let callback_ptr = Box::into_raw(callback_ctx);

        let backend = Arc::clone(&self.backend);
        let session = backend.session_start(
            engine,
            Some(bridge_event_callback),
            callback_ptr.cast(),
        )?;

        let shared = Arc::new(Mutex::new(SessionSharedState::default()));
        let utterance_id = "utt_1".to_string();
        let dispatcher = spawn_event_dispatcher(
            config.session_id.clone(),
            utterance_id.clone(),
            events,
            event_rx,
            cancelled.clone(),
            Arc::clone(&shared),
        );

        Ok(Box::new(FluidAudioSpeechSession {
            backend,
            session,
            _callback_ctx: CallbackContextPtr::new(callback_ptr),
            config,
            utterance_id,
            cancelled,
            shared,
            dispatcher: Some(dispatcher),
        }))
    }
}

struct FluidAudioSpeechSession {
    backend: Arc<dyn VoiceBridgeBackend>,
    session: LatticeVoiceSession,
    _callback_ctx: CallbackContextPtr,
    config: SpeechSessionConfig,
    utterance_id: String,
    cancelled: Arc<AtomicBool>,
    shared: Arc<Mutex<SessionSharedState>>,
    dispatcher: Option<JoinHandle<()>>,
}

impl FluidAudioSpeechSession {
    fn decode_f32_samples(chunk: &AudioChunk) -> Result<Vec<f32>, SpeechError> {
        if chunk.sample_format != AudioSampleFormat::F32 {
            return Err(SpeechError::provider(format!(
                "unsupported sample format {:?} (expected F32)",
                chunk.sample_format
            )));
        }
        if chunk.channels != 1 {
            return Err(SpeechError::provider(format!(
                "unsupported channel count {} (expected mono)",
                chunk.channels
            )));
        }
        if chunk.sample_rate_hz != 16_000 {
            return Err(SpeechError::provider(format!(
                "unsupported sample rate {} Hz (expected 16000)",
                chunk.sample_rate_hz
            )));
        }
        if chunk.payload.len() % 4 != 0 {
            return Err(SpeechError::provider("F32 payload length is not aligned"));
        }

        let mut samples = Vec::with_capacity(chunk.payload.len() / 4);
        for bytes in chunk.payload.chunks_exact(4) {
            samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        }
        Ok(samples)
    }

    async fn wait_for_final(&self, deadline: Instant) -> Result<String, SpeechError> {
        while Instant::now() < deadline {
            if let Ok(guard) = self.shared.lock() {
                if let Some(text) = guard.final_text.clone() {
                    return Ok(text);
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Err(SpeechError::provider(
            "timed out waiting for final transcript callback",
        ))
    }
}

#[async_trait]
impl SpeechSession for FluidAudioSpeechSession {
    async fn push_audio(&mut self, chunk: AudioChunk) -> Result<(), SpeechError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(SpeechError::provider("session was cancelled"));
        }

        let samples = Self::decode_f32_samples(&chunk)?;
        if samples.is_empty() {
            return Ok(());
        }

        let backend = Arc::clone(&self.backend);
        let session = self.session;
        tokio::task::spawn_blocking(move || backend.session_push_audio(session, &samples))
            .await
            .map_err(|err| SpeechError::provider(format!("push_audio task failed: {err}")))??;
        Ok(())
    }

    async fn finish_utterance(&mut self) -> Result<FinalTranscript, SpeechError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(SpeechError::provider("session was cancelled"));
        }

        let backend = Arc::clone(&self.backend);
        let session = self.session;
        let started = Instant::now();

        tokio::task::spawn_blocking(move || backend.session_finish_utterance(session))
            .await
            .map_err(|err| SpeechError::provider(format!("finish_utterance task failed: {err}")))??;

        let processing_ms = started.elapsed().as_millis() as u64;
        let final_text = self
            .wait_for_final(Instant::now() + Duration::from_secs(5))
            .await?;

        let (revision, _) = self
            .shared
            .lock()
            .map(|guard| (guard.revision, guard.final_text.clone()))
            .map_err(|_| SpeechError::provider("session state lock poisoned"))?;

        Ok(FinalTranscript {
            session_id: self.config.session_id.clone(),
            utterance_id: self.utterance_id.clone(),
            replaces_revision: revision,
            text: final_text,
            decode_mode: DecodeMode::Offline,
            duration_ms: 0,
            processing_ms,
        })
    }

    async fn cancel(mut self: Box<Self>) -> Result<(), SpeechError> {
        self.cancelled.store(true, Ordering::Release);
        if let Some(handle) = self.dispatcher.take() {
            handle.abort();
        }

        let backend = Arc::clone(&self.backend);
        let session = self.session;
        tokio::task::spawn_blocking(move || backend.session_cancel(session))
            .await
            .map_err(|err| SpeechError::provider(format!("cancel task failed: {err}")))??;
        Ok(())
    }
}

impl Drop for FluidAudioSpeechSession {
    fn drop(&mut self) {
        if let Some(handle) = self.dispatcher.take() {
            handle.abort();
        }
        self.backend.session_destroy(self.session);
    }
}

fn spawn_event_dispatcher(
    session_id: String,
    utterance_id: String,
    events: SpeechEventSender,
    event_rx: std::sync::mpsc::Receiver<OwnedBridgeEvent>,
    cancelled: Arc<AtomicBool>,
    shared: Arc<Mutex<SessionSharedState>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv() {
            if cancelled.load(Ordering::Acquire) {
                continue;
            }

            let revision = {
                let mut guard = match shared.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                match event.kind {
                    LatticeVoiceEventKind::Partial | LatticeVoiceEventKind::Stable => {
                        guard.revision += 1;
                    }
                    LatticeVoiceEventKind::Final => {
                        guard.revision += 1;
                        guard.final_text = Some(event.text.clone());
                    }
                    LatticeVoiceEventKind::Error => {}
                }
                guard.revision
            };

            let result = match event.kind {
                LatticeVoiceEventKind::Partial => events.send(VoiceEvent::PartialTranscript(
                    PartialTranscriptPayload {
                        session_id: session_id.clone(),
                        utterance_id: utterance_id.clone(),
                        revision,
                        text: event.text,
                        stable_prefix_bytes: event.stable_prefix_bytes,
                        started_at_ms: 0,
                        ended_at_ms: 0,
                    },
                )),
                LatticeVoiceEventKind::Stable => events.send(VoiceEvent::StableTranscript(
                    StableTranscriptPayload {
                        session_id: session_id.clone(),
                        utterance_id: utterance_id.clone(),
                        revision,
                        text: event.text,
                        stable_prefix_bytes: event.stable_prefix_bytes,
                    },
                )),
                LatticeVoiceEventKind::Final => events.send(VoiceEvent::FinalTranscript(
                    FinalTranscript {
                        session_id: session_id.clone(),
                        utterance_id: utterance_id.clone(),
                        replaces_revision: revision,
                        text: event.text,
                        decode_mode: DecodeMode::Offline,
                        duration_ms: 0,
                        processing_ms: 0,
                    },
                )),
                LatticeVoiceEventKind::Error => {
                    if event.error_code == 0 {
                        continue;
                    }
                    events.send(VoiceEvent::SessionFailed {
                        session_id: session_id.clone(),
                        message: event.text,
                        state: lattice_voice::TranscriptionSessionState::Failed,
                    })
                }
            };

            if result.is_err() {
                break;
            }
        }
    })
}

/// Default FluidAudio model cache: env override, then M0 research cache if present.
pub fn default_model_cache_dir() -> PathBuf {
    if let Ok(path) = std::env::var("LATTICE_VOICE_MODEL_CACHE") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../research/voice-m0-fluidaudio/.cache/Models")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use lattice_voice::{AudioSampleFormat, SessionContext, VoiceEvent};

    use super::*;
    use crate::bridge::MockBridge;

    fn sample_config() -> SpeechSessionConfig {
        SpeechSessionConfig {
            session_id: "voice_test".into(),
            language: Some("en".into()),
            context: SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                command_mode: false,
            },
        }
    }

    fn f32_chunk(session_id: &str, sequence: u64, samples: &[f32]) -> AudioChunk {
        let mut payload = Vec::with_capacity(samples.len() * 4);
        for sample in samples {
            payload.extend_from_slice(&sample.to_le_bytes());
        }
        AudioChunk {
            session_id: session_id.into(),
            sequence,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: Bytes::from(payload),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mock_provider_streams_partial_and_final() {
        let bridge = Arc::new(MockBridge::new(LATTICE_VOICE_BRIDGE_ABI_VERSION));
        let provider = FluidAudioSpeechProvider::with_backend(bridge, PathBuf::new());

        provider
            .prepare(PrepareModelRequest {
                model_id: DEFAULT_MODEL_ID.into(),
                warm: true,
            })
            .await
            .unwrap();

        let (events, mut rx) = SpeechEventSender::pair();
        let mut session = provider
            .start_session(sample_config(), events)
            .await
            .unwrap();

        session
            .push_audio(f32_chunk("voice_test", 0, &[0.0, 0.1, -0.1]))
            .await
            .unwrap();

        let final_transcript = session.finish_utterance().await.unwrap();
        assert_eq!(final_transcript.text, "mock final");

        let mut saw_partial = false;
        let mut saw_final = false;
        while let Ok(event) = rx.try_recv() {
            match event {
                VoiceEvent::PartialTranscript(_) => saw_partial = true,
                VoiceEvent::FinalTranscript(_) => saw_final = true,
                _ => {}
            }
        }
        assert!(saw_partial);
        assert!(saw_final);
    }

    #[tokio::test]
    async fn abi_mismatch_rejects_prepare() {
        let bridge = Arc::new(MockBridge::new(99));
        let provider = FluidAudioSpeechProvider::with_backend(bridge, PathBuf::new());
        let err = provider
            .prepare(PrepareModelRequest {
                model_id: DEFAULT_MODEL_ID.into(),
                warm: false,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("ABI mismatch"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn engine_create_destroy_ordering() {
        let bridge = Arc::new(MockBridge::new(LATTICE_VOICE_BRIDGE_ABI_VERSION));
        let provider = FluidAudioSpeechProvider::with_backend(bridge, PathBuf::new());
        provider
            .prepare(PrepareModelRequest {
                model_id: DEFAULT_MODEL_ID.into(),
                warm: true,
            })
            .await
            .unwrap();
        drop(provider);
    }
}
