use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::SpeechError;
use crate::protocol::{
    AudioChunk, FinalTranscript, ModelStatus, PrepareModelRequest, SpeechCapabilities,
    SpeechSessionConfig, VoiceEvent,
};

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
#[derive(Debug)]
pub struct NullSpeechProvider {
    capabilities: SpeechCapabilities,
}

impl NullSpeechProvider {
    pub fn new() -> Self {
        Self {
            capabilities: SpeechCapabilities {
                streaming: true,
                partial_transcripts: true,
                offline_final_decode: true,
                punctuation: false,
                word_timestamps: false,
                language_detection: false,
                vocabulary_biasing: false,
                endpoint_detection: false,
                supported_languages: vec!["en".into()],
            },
        }
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
        Ok(Box::new(NullSpeechSession::new(config, events)))
    }
}

struct NullSpeechSession {
    config: SpeechSessionConfig,
    events: SpeechEventSender,
    next_sequence: u64,
    revision: u64,
    utterance_id: String,
    partial_text: String,
}

impl NullSpeechSession {
    fn new(config: SpeechSessionConfig, events: SpeechEventSender) -> Self {
        Self {
            config,
            events,
            next_sequence: 0,
            revision: 0,
            utterance_id: "utt_1".into(),
            partial_text: String::new(),
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

        let final_transcript = FinalTranscript {
            session_id: self.config.session_id.clone(),
            utterance_id: self.utterance_id.clone(),
            replaces_revision: self.revision,
            text: final_text,
            decode_mode: crate::protocol::DecodeMode::Offline,
            duration_ms: 0,
            processing_ms: 0,
        };

        Ok(final_transcript)
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
    use crate::protocol::{
        AudioSampleFormat, SessionContext, VoiceEvent,
    };

    fn sample_chunk(session_id: &str, sequence: u64) -> AudioChunk {
        AudioChunk {
            session_id: session_id.into(),
            sequence,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: Bytes::from_static(b"pcm"),
        }
    }

    #[tokio::test]
    async fn null_provider_rejects_sequence_gaps() {
        let provider = NullSpeechProvider::new();
        let (events, _rx) = SpeechEventSender::pair();
        let config = SpeechSessionConfig {
            session_id: "voice_1".into(),
            language: Some("en".into()),
            context: SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                command_mode: false,
            },
        };

        let mut session = provider.start_session(config, events).await.unwrap();
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
    async fn null_provider_emits_partial_then_final() {
        let provider = Arc::new(NullSpeechProvider::new());
        let (events, mut rx) = SpeechEventSender::pair();
        let config = SpeechSessionConfig {
            session_id: "voice_1".into(),
            language: Some("en".into()),
            context: SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                command_mode: false,
            },
        };

        let mut session = provider.start_session(config, events).await.unwrap();
        session
            .push_audio(sample_chunk("voice_1", 0))
            .await
            .unwrap();
        let final_transcript = session.finish_utterance().await.unwrap();

        assert!(matches!(rx.recv().await.unwrap(), VoiceEvent::PartialTranscript(_)));
        assert!(final_transcript.text.contains("final"));
    }
}
