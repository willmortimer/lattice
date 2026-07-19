use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::SpeechError;
use crate::independent_final::{
    commit_final_transcript, IndependentFinalPolicy, OfflineRedecodeBackend,
    UnimplementedOfflineRedecode,
};
use crate::protocol::{
    AudioChunk, FinalTranscript, ModelStatus, PrepareModelRequest, SpeechCapabilities,
    SpeechSessionConfig, VoiceEvent,
};
use crate::utterance_buffer::UtteranceAudioBuffer;

/// Delivers streaming voice events to a local subscriber.
#[derive(Clone, Debug)]
pub struct SpeechEventSender {
    tx: mpsc::UnboundedSender<VoiceEvent>,
}

impl SpeechEventSender {
    pub fn new(tx: mpsc::UnboundedSender<VoiceEvent>) -> Self {
        Self { tx }
    }

    pub fn pair() -> (Self, mpsc::UnboundedReceiver<VoiceEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self::new(tx), rx)
    }

    pub fn send(&self, event: VoiceEvent) -> Result<(), SpeechError> {
        self.tx
            .send(event)
            .map_err(|_| SpeechError::EventSubscriberDisconnected)
    }
}

/// Provider surface shared by in-process and daemon implementations.
#[async_trait]
pub trait SpeechProvider: Send + Sync {
    fn capabilities(&self) -> SpeechCapabilities;

    async fn prepare(
        &self,
        request: PrepareModelRequest,
    ) -> Result<ModelStatus, SpeechError>;

    async fn start_session(
        &self,
        config: SpeechSessionConfig,
        events: SpeechEventSender,
    ) -> Result<Box<dyn SpeechSession>, SpeechError>;
}

/// Per-session streaming recognition handle.
#[async_trait]
pub trait SpeechSession: Send {
    async fn push_audio(&mut self, chunk: AudioChunk) -> Result<(), SpeechError>;

    async fn finish_utterance(&mut self) -> Result<FinalTranscript, SpeechError>;

    async fn cancel(self: Box<Self>) -> Result<(), SpeechError>;
}

/// Test-only provider that validates chunk sequencing and emits fake transcripts.
///
/// Buffers full-utterance PCM like production sessions. The committed final stays
/// [`crate::protocol::FinalizationMode::StreamingFlush`] unless an implemented
/// offline backend is selected via policy (see `LATTICE_VOICE_INDEPENDENT_FINAL`).
pub struct NullSpeechProvider {
    capabilities: SpeechCapabilities,
    offline: Arc<dyn OfflineRedecodeBackend>,
    /// When set, overrides env/capability policy (unit tests).
    policy_override: Option<IndependentFinalPolicy>,
}

impl NullSpeechProvider {
    pub fn new() -> Self {
        Self {
            capabilities: SpeechCapabilities {
                streaming: true,
                partial_transcripts: true,
                finalization_mode: crate::protocol::FinalizationMode::StreamingFlush,
                punctuation: false,
                word_timestamps: false,
                language_detection: false,
                vocabulary_biasing: false,
                endpoint_detection: false,
                supported_languages: vec!["en".into()],
            },
            offline: Arc::new(UnimplementedOfflineRedecode),
            policy_override: None,
        }
    }

    /// Inject an offline re-decode backend (tests / harnesses).
    pub fn with_offline(mut self, offline: Arc<dyn OfflineRedecodeBackend>) -> Self {
        self.offline = offline;
        self
    }

    /// Override independent-final attempt policy (avoids process-wide env in tests).
    pub fn with_policy(mut self, policy: IndependentFinalPolicy) -> Self {
        self.policy_override = Some(policy);
        self
    }
}

impl Default for NullSpeechProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SpeechProvider for NullSpeechProvider {
    fn capabilities(&self) -> SpeechCapabilities {
        self.capabilities.clone()
    }

    async fn prepare(
        &self,
        _request: PrepareModelRequest,
    ) -> Result<ModelStatus, SpeechError> {
        Ok(ModelStatus {
            state: crate::protocol::ModelState::Ready,
            model_version: Some("null-0.1".into()),
            provider_version: Some("null-provider".into()),
            message: None,
        })
    }

    async fn start_session(
        &self,
        config: SpeechSessionConfig,
        events: SpeechEventSender,
    ) -> Result<Box<dyn SpeechSession>, SpeechError> {
        let policy = self.policy_override.unwrap_or_else(|| {
            IndependentFinalPolicy::from_env_and_capabilities(&self.capabilities)
        });
        Ok(Box::new(NullSpeechSession::new(
            config,
            events,
            Arc::clone(&self.offline),
            policy,
        )))
    }
}

struct NullSpeechSession {
    config: SpeechSessionConfig,
    events: SpeechEventSender,
    next_sequence: u64,
    revision: u64,
    utterance_id: String,
    partial_text: String,
    utterance_audio: UtteranceAudioBuffer,
    offline: Arc<dyn OfflineRedecodeBackend>,
    policy: IndependentFinalPolicy,
}

impl NullSpeechSession {
    fn new(
        config: SpeechSessionConfig,
        events: SpeechEventSender,
        offline: Arc<dyn OfflineRedecodeBackend>,
        policy: IndependentFinalPolicy,
    ) -> Self {
        Self {
            config,
            events,
            next_sequence: 0,
            revision: 0,
            utterance_id: "utt_1".into(),
            partial_text: String::new(),
            utterance_audio: UtteranceAudioBuffer::new(),
            offline,
            policy,
        }
    }

    fn emit_partial(&mut self, text: &str) -> Result<(), SpeechError> {
        self.revision += 1;
        self.partial_text = text.to_string();
        self.events.send(VoiceEvent::PartialTranscript(
            crate::protocol::PartialTranscriptPayload {
                session_id: self.config.session_id.clone(),
                utterance_id: self.utterance_id.clone(),
                revision: self.revision,
                text: self.partial_text.clone(),
                stable_prefix_bytes: 0,
                started_at_ms: 0,
                ended_at_ms: 0,
            },
        ))
    }
}

#[async_trait]
impl SpeechSession for NullSpeechSession {
    async fn push_audio(&mut self, chunk: AudioChunk) -> Result<(), SpeechError> {
        if chunk.session_id != self.config.session_id {
            return Err(SpeechError::provider("audio chunk session mismatch"));
        }

        if chunk.sequence != self.next_sequence {
            return Err(SpeechError::SequenceGap {
                session_id: chunk.session_id,
                expected: self.next_sequence,
                received: chunk.sequence,
            });
        }

        self.next_sequence += 1;
        self.utterance_audio.push_chunk(&chunk)?;

        if chunk.payload.is_empty() {
            return Ok(());
        }

        let partial = format!("partial-{}", self.next_sequence);
        self.emit_partial(&partial)
    }

    async fn finish_utterance(&mut self) -> Result<FinalTranscript, SpeechError> {
        let final_text = if self.partial_text.is_empty() {
            "final transcript".into()
        } else {
            format!("{} final", self.partial_text)
        };

        let streaming_flush = FinalTranscript {
            session_id: self.config.session_id.clone(),
            utterance_id: self.utterance_id.clone(),
            replaces_revision: self.revision,
            text: final_text,
            raw_text: None,
            corrections: Vec::new(),
            finalization_mode: crate::protocol::FinalizationMode::StreamingFlush,
            duration_ms: 0,
            processing_ms: 0,
        };

        let frozen = std::mem::take(&mut self.utterance_audio).freeze();
        Ok(commit_final_transcript(
            streaming_flush,
            self.policy,
            self.offline.as_ref(),
            &frozen,
        ))
    }

    async fn cancel(self: Box<Self>) -> Result<(), SpeechError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use bytes::Bytes;
    use crate::independent_final::{
        FakeIndependentOfflineRedecode, IndependentFinalPolicy,
    };
    use crate::protocol::{AudioSampleFormat, FinalizationMode, SessionContext, VoiceEvent};

    fn sample_chunk(session_id: &str, sequence: u64) -> AudioChunk {
        AudioChunk {
            session_id: session_id.into(),
            sequence,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: Bytes::from_static(b"pcm\0"),
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

    fn sample_config(session_id: &str) -> SpeechSessionConfig {
        SpeechSessionConfig {
            session_id: session_id.into(),
            language: Some("en".into()),
            context: SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                known_paths: Vec::new(),
                command_mode: false,
            },
        }
    }

    #[tokio::test]
    async fn null_provider_rejects_sequence_gaps() {
        let provider = NullSpeechProvider::new();
        let (events, _rx) = SpeechEventSender::pair();
        let mut session = provider
            .start_session(sample_config("voice_1"), events)
            .await
            .unwrap();
        session
            .push_audio(sample_chunk("voice_1", 0))
            .await
            .unwrap();

        let err = session
            .push_audio(sample_chunk("voice_1", 2))
            .await
            .unwrap_err();
        assert!(matches!(err, SpeechError::SequenceGap { .. }));
    }

    #[tokio::test]
    async fn null_provider_emits_partial_then_streaming_flush_final() {
        let provider = Arc::new(NullSpeechProvider::new());
        let (events, mut rx) = SpeechEventSender::pair();
        let mut session = provider
            .start_session(sample_config("voice_1"), events)
            .await
            .unwrap();
        session
            .push_audio(sample_chunk("voice_1", 0))
            .await
            .unwrap();
        let final_transcript = session.finish_utterance().await.unwrap();

        assert!(matches!(
            rx.recv().await.unwrap(),
            VoiceEvent::PartialTranscript(_)
        ));
        assert_eq!(
            final_transcript.finalization_mode,
            FinalizationMode::StreamingFlush
        );
    }

    #[tokio::test]
    async fn null_provider_buffers_frames_and_keeps_flush_without_backend() {
        let provider = NullSpeechProvider::new().with_policy(IndependentFinalPolicy::for_tests(
            true, false,
        ));
        let (events, _rx) = SpeechEventSender::pair();
        let mut session = provider
            .start_session(sample_config("voice_buf"), events)
            .await
            .unwrap();

        session
            .push_audio(f32_chunk("voice_buf", 0, &[0.0, 0.25]))
            .await
            .unwrap();
        session
            .push_audio(f32_chunk("voice_buf", 1, &[0.5, 0.75]))
            .await
            .unwrap();

        let final_transcript = session.finish_utterance().await.unwrap();
        // Env requested independent final, but stub backend → honest StreamingFlush.
        assert_eq!(
            final_transcript.finalization_mode,
            FinalizationMode::StreamingFlush
        );
        assert!(final_transcript.text.ends_with(" final"));
    }

    #[tokio::test]
    async fn null_provider_commits_independent_when_backend_implemented() {
        let backend = Arc::new(FakeIndependentOfflineRedecode::default());
        let provider = NullSpeechProvider::new()
            .with_offline(backend)
            .with_policy(IndependentFinalPolicy::for_tests(true, false));
        let (events, _rx) = SpeechEventSender::pair();
        let mut session = provider
            .start_session(sample_config("voice_ind"), events)
            .await
            .unwrap();

        session
            .push_audio(f32_chunk("voice_ind", 0, &[0.1, 0.2]))
            .await
            .unwrap();
        session
            .push_audio(f32_chunk("voice_ind", 1, &[0.3]))
            .await
            .unwrap();

        let final_transcript = session.finish_utterance().await.unwrap();
        assert_eq!(
            final_transcript.finalization_mode,
            FinalizationMode::IndependentOfflineRedecode
        );
        assert_eq!(final_transcript.text, "independent-frames-2-samples-3");
    }
}
