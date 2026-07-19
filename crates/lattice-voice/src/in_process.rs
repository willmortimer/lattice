use std::collections::HashMap;
use std::sync::Arc;

use crate::endpoint::{decode_f32_le, EndpointPolicy, EndpointSignal};
use crate::error::SpeechError;
use crate::normalize::{normalize_final_transcript, NormalizationContext};
use crate::protocol::{
    AudioChunk, AudioSampleFormat, FinishUtteranceRequest, PROTOCOL_VERSION, SessionContext,
    TranscriptionSessionState, UpdateSessionContextRequest, VoiceEvent, VoiceRequest,
    VoiceSessionId,
};
use crate::provider::{SpeechEventSender, SpeechProvider, SpeechSession};
use crate::session::SessionStateMachine;

struct ActiveSession {
    state: SessionStateMachine,
    provider_session: Box<dyn SpeechSession>,
    session_context: SessionContext,
    next_sequence: u64,
    last_revision: u64,
    utterance_index: u64,
    endpoint_policy: EndpointPolicy,
    auto_finalize_on_endpoint: bool,
}

impl ActiveSession {
    fn utterance_id(&self) -> String {
        format!("utt_{}", self.utterance_index)
    }
}

/// In-process dispatcher for the daemon-compatible voice protocol.
pub struct InProcessVoiceService {
    provider: Arc<dyn SpeechProvider>,
    sessions: HashMap<VoiceSessionId, ActiveSession>,
}

impl InProcessVoiceService {
    pub fn new(provider: Arc<dyn SpeechProvider>) -> Self {
        Self {
            provider,
            sessions: HashMap::new(),
        }
    }

    pub async fn handle_request(
        &mut self,
        request: VoiceRequest,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        match request {
            VoiceRequest::PrepareModel(request) => {
                let status = self.provider.prepare(request).await?;
                events.send(VoiceEvent::ModelStatusChanged(status))?;
            }
            VoiceRequest::GetVoiceCapabilities => {
                let capabilities = self.provider.capabilities();
                events.send(VoiceEvent::SessionReady {
                    session_id: "capabilities".into(),
                    protocol_version: PROTOCOL_VERSION,
                    capabilities,
                })?;
            }
            VoiceRequest::StartVoiceSession(request) => {
                self.start_session(request.config, events).await?;
            }
            VoiceRequest::PushAudioChunk(chunk) => {
                self.push_audio(chunk, events).await?;
            }
            VoiceRequest::FinishUtterance(request) => {
                self.finish_utterance(request, events).await?;
            }
            VoiceRequest::UpdateSessionContext(request) => {
                self.update_session_context(request)?;
            }
            VoiceRequest::CancelVoiceSession(request) => {
                self.cancel_session(&request.session_id, events).await?;
            }
            VoiceRequest::EndVoiceSession(request) => {
                self.end_session(&request.session_id, events).await?;
            }
        }

        Ok(())
    }

    async fn start_session(
        &mut self,
        config: crate::protocol::SpeechSessionConfig,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        let session_id = config.session_id.clone();
        if self.sessions.contains_key(&session_id) {
            return Err(SpeechError::provider(format!(
                "session already exists: {session_id}"
            )));
        }

        let mut state = SessionStateMachine::new(session_id.clone());
        state.transition(TranscriptionSessionState::Preparing)?;

        let endpoint_options = config.endpoint.clone();
        let auto_finalize_on_endpoint = endpoint_options.auto_finalize_enabled();
        let endpoint_policy = EndpointPolicy::new(&endpoint_options);

        let provider_session = self
            .provider
            .start_session(config.clone(), events.clone())
            .await?;

        state.transition(TranscriptionSessionState::Ready)?;
        state.transition(TranscriptionSessionState::Listening)?;

        events.send(VoiceEvent::SessionReady {
            session_id: session_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            capabilities: self.provider.capabilities(),
        })?;

        self.sessions.insert(
            session_id,
            ActiveSession {
                state,
                provider_session,
                session_context: config.context.clone(),
                next_sequence: 0,
                last_revision: 0,
                utterance_index: 1,
                endpoint_policy,
                auto_finalize_on_endpoint,
            },
        );

        Ok(())
    }

    fn update_session_context(
        &mut self,
        request: UpdateSessionContextRequest,
    ) -> Result<(), SpeechError> {
        let session = self.session_mut(&request.session_id)?;
        session.session_context = request.context;
        Ok(())
    }

    /// Returns the stored session context for tests and diagnostics.
    pub fn session_context(
        &self,
        session_id: &VoiceSessionId,
    ) -> Result<SessionContext, SpeechError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SpeechError::SessionNotFound {
                session_id: session_id.clone(),
            })?;
        Ok(session.session_context.clone())
    }

    async fn push_audio(
        &mut self,
        chunk: AudioChunk,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        let session_id = chunk.session_id.clone();
        let (signal, utterance_id, should_auto_finalize) = {
            let session = self.session_mut(&session_id)?;

            if chunk.sequence != session.next_sequence {
                return Err(SpeechError::SequenceGap {
                    session_id: chunk.session_id.clone(),
                    expected: session.next_sequence,
                    received: chunk.sequence,
                });
            }

            let signal = apply_endpoint_policy(session, &chunk);
            let utterance_id = session.utterance_id();
            let should_auto_finalize =
                session.auto_finalize_on_endpoint && matches!(signal, EndpointSignal::Endpoint(_));
            (signal, utterance_id, should_auto_finalize)
        };

        match signal {
            EndpointSignal::None => {}
            EndpointSignal::SpeechStarted => {
                let session = self.session_mut(&session_id)?;
                if session.state.state() == TranscriptionSessionState::Listening {
                    session
                        .state
                        .transition(TranscriptionSessionState::SpeechActive)?;
                }
                events.send(VoiceEvent::SpeechStarted {
                    session_id: session_id.clone(),
                    utterance_id: utterance_id.clone(),
                    started_at_ms: 0,
                })?;
            }
            EndpointSignal::Endpoint(reason) => {
                events.send(VoiceEvent::EndpointDetected {
                    session_id: session_id.clone(),
                    utterance_id: utterance_id.clone(),
                    ended_at_ms: 0,
                    reason,
                })?;
            }
        }

        {
            let session = self.session_mut(&session_id)?;
            // Hold-to-talk without VAD speech: still accept audio while Listening.
            if session.state.state() == TranscriptionSessionState::Listening
                && !matches!(signal, EndpointSignal::SpeechStarted)
            {
                // Keep Listening until explicit speech onset or finish.
            }
            session.provider_session.push_audio(chunk).await?;
            session.next_sequence += 1;
        }

        if should_auto_finalize {
            self.finish_utterance(
                FinishUtteranceRequest {
                    session_id,
                    utterance_id,
                },
                events,
            )
            .await?;
        }

        Ok(())
    }

    async fn finish_utterance(
        &mut self,
        request: FinishUtteranceRequest,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        let continuous = {
            let session = self.session_mut(&request.session_id)?;
            if session.state.state() == TranscriptionSessionState::Listening {
                // Explicit PTT release with no VAD speech onset still finalizes.
                session
                    .state
                    .transition(TranscriptionSessionState::SpeechActive)?;
            }
            session
                .state
                .transition(TranscriptionSessionState::Finalizing)?;
            session.auto_finalize_on_endpoint
        };

        let (final_transcript, session_id) = {
            let session = self.session_mut(&request.session_id)?;
            let final_transcript = session.provider_session.finish_utterance().await?;
            validate_revision(
                &request.session_id,
                session.last_revision,
                final_transcript.replaces_revision,
            )?;
            session.last_revision = final_transcript.replaces_revision;

            let normalization_context = NormalizationContext::from(&session.session_context);
            let final_transcript =
                normalize_final_transcript(final_transcript, &normalization_context);
            (final_transcript, request.session_id.clone())
        };

        events.send(VoiceEvent::FinalTranscript(final_transcript))?;

        let session = self.session_mut(&session_id)?;
        session.endpoint_policy.reset();
        if continuous {
            session
                .state
                .transition(TranscriptionSessionState::Listening)?;
            session.utterance_index = session.utterance_index.saturating_add(1);
        } else {
            session
                .state
                .transition(TranscriptionSessionState::Completed)?;
            events.send(VoiceEvent::SessionCompleted {
                session_id: session_id.clone(),
                state: session.state.state(),
            })?;
        }

        Ok(())
    }

    async fn cancel_session(
        &mut self,
        session_id: &VoiceSessionId,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        let mut session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| SpeechError::SessionNotFound {
                session_id: session_id.clone(),
            })?;

        if !session.state.is_terminal() {
            let _ = session.state.cancel();
        }

        session.provider_session.cancel().await?;
        events.send(VoiceEvent::SessionCompleted {
            session_id: session_id.clone(),
            state: TranscriptionSessionState::Cancelled,
        })?;

        Ok(())
    }

    async fn end_session(
        &mut self,
        session_id: &VoiceSessionId,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        if self.sessions.contains_key(session_id) {
            self.cancel_session(session_id, events).await?;
        }
        Ok(())
    }

    fn session_mut(&mut self, session_id: &VoiceSessionId) -> Result<&mut ActiveSession, SpeechError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SpeechError::SessionNotFound {
                session_id: session_id.clone(),
            })?;

        if session.state.is_terminal() {
            return Err(SpeechError::SessionTerminal {
                session_id: session_id.clone(),
                state: session.state.state(),
            });
        }

        Ok(session)
    }
}

fn apply_endpoint_policy(session: &mut ActiveSession, chunk: &AudioChunk) -> EndpointSignal {
    if chunk.payload.is_empty() {
        return EndpointSignal::None;
    }

    let samples = match chunk.sample_format {
        AudioSampleFormat::F32 => decode_f32_le(&chunk.payload),
        AudioSampleFormat::I16Le => decode_i16_le_as_f32(&chunk.payload),
    };
    session
        .endpoint_policy
        .push_samples(&samples, chunk.sample_rate_hz)
}

fn decode_i16_le_as_f32(payload: &[u8]) -> Vec<f32> {
    let mut samples = Vec::with_capacity(payload.len() / 2);
    for bytes in payload.chunks_exact(2) {
        let sample = i16::from_le_bytes([bytes[0], bytes[1]]);
        samples.push(f32::from(sample) / 32768.0);
    }
    samples
}

fn validate_revision(
    session_id: &VoiceSessionId,
    last_revision: u64,
    received_revision: u64,
) -> Result<(), SpeechError> {
    if received_revision < last_revision {
        return Err(SpeechError::RevisionOutOfOrder {
            session_id: session_id.clone(),
            last_revision,
            received_revision,
        });
    }

    Ok(())
}

pub fn record_transcript_revision(
    session_id: &VoiceSessionId,
    last_revision: u64,
    received_revision: u64,
) -> Result<u64, SpeechError> {
    validate_revision(session_id, last_revision, received_revision)?;
    Ok(received_revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use crate::endpoint::EndpointOptions;
    use crate::protocol::{
        EndpointReason, SessionContext, SpeechSessionConfig, StartVoiceSessionRequest,
        UpdateSessionContextRequest,
    };
    use crate::provider::NullSpeechProvider;

    fn sample_chunk(session_id: &str, sequence: u64) -> AudioChunk {
        // Quiet PCM — hold-to-talk still accepts audio without VAD onset.
        AudioChunk {
            session_id: session_id.into(),
            sequence,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: Bytes::from_static(&[0, 0, 0, 0]),
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

    fn speech_samples(ms: u32, amplitude: f32) -> Vec<f32> {
        let n = (u64::from(ms) * 16_000 / 1000) as usize;
        vec![amplitude; n]
    }

    fn base_config(session_id: &str) -> SpeechSessionConfig {
        SpeechSessionConfig {
            session_id: session_id.into(),
            language: Some("en".into()),
            context: SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                known_paths: Vec::new(),
                command_mode: false,
            },
            endpoint: EndpointOptions::default(),
        }
    }

    #[tokio::test]
    async fn in_process_partial_to_final_happy_path() {
        let provider = Arc::new(NullSpeechProvider::new());
        let mut service = InProcessVoiceService::new(provider);
        let (events, mut rx) = SpeechEventSender::pair();

        service
            .handle_request(
                VoiceRequest::StartVoiceSession(StartVoiceSessionRequest {
                    config: base_config("voice_1"),
                }),
                &events,
            )
            .await
            .unwrap();

        let ready = rx.recv().await.unwrap();
        assert!(matches!(ready, VoiceEvent::SessionReady { .. }));

        // Hold-to-talk: quiet audio does not require VAD SpeechStarted.
        service
            .handle_request(
                VoiceRequest::PushAudioChunk(sample_chunk("voice_1", 0)),
                &events,
            )
            .await
            .unwrap();

        let partial = rx.recv().await.unwrap();
        assert!(matches!(partial, VoiceEvent::PartialTranscript(_)));

        service
            .handle_request(
                VoiceRequest::FinishUtterance(FinishUtteranceRequest {
                    session_id: "voice_1".into(),
                    utterance_id: "utt_1".into(),
                }),
                &events,
            )
            .await
            .unwrap();

        let final_event = rx.recv().await.unwrap();
        match final_event {
            VoiceEvent::FinalTranscript(transcript) => {
                assert_eq!(
                    transcript.finalization_mode,
                    crate::protocol::FinalizationMode::StreamingFlush
                );
            }
            other => panic!("expected FinalTranscript, got {other:?}"),
        }

        let completed = rx.recv().await.unwrap();
        assert!(matches!(completed, VoiceEvent::SessionCompleted { .. }));
    }

    #[tokio::test]
    async fn continuous_auto_finalizes_on_silence_endpoint() {
        let provider = Arc::new(NullSpeechProvider::new());
        assert!(provider.capabilities().endpoint_detection);

        let mut service = InProcessVoiceService::new(provider);
        let (events, mut rx) = SpeechEventSender::pair();

        let mut config = base_config("voice_cont");
        config.endpoint = EndpointOptions {
            auto_finalize_on_endpoint: true,
            silence_debounce_ms: 100,
            max_utterance_ms: 60_000,
        };

        service
            .handle_request(
                VoiceRequest::StartVoiceSession(StartVoiceSessionRequest { config }),
                &events,
            )
            .await
            .unwrap();
        let _ = rx.recv().await.unwrap(); // SessionReady

        service
            .handle_request(
                VoiceRequest::PushAudioChunk(f32_chunk(
                    "voice_cont",
                    0,
                    &speech_samples(50, 0.2),
                )),
                &events,
            )
            .await
            .unwrap();

        let speech_started = rx.recv().await.unwrap();
        assert!(matches!(speech_started, VoiceEvent::SpeechStarted { .. }));
        let partial = rx.recv().await.unwrap();
        assert!(matches!(partial, VoiceEvent::PartialTranscript(_)));

        service
            .handle_request(
                VoiceRequest::PushAudioChunk(f32_chunk(
                    "voice_cont",
                    1,
                    &speech_samples(120, 0.0),
                )),
                &events,
            )
            .await
            .unwrap();

        let mut saw_endpoint = false;
        let mut saw_final = false;
        let mut saw_completed = false;
        while let Ok(event) = rx.try_recv() {
            match event {
                VoiceEvent::EndpointDetected {
                    reason: EndpointReason::SilenceDebounce,
                    ..
                } => saw_endpoint = true,
                VoiceEvent::FinalTranscript(_) => saw_final = true,
                VoiceEvent::SessionCompleted { .. } => saw_completed = true,
                VoiceEvent::PartialTranscript(_) => {}
                other => panic!("unexpected event: {other:?}"),
            }
        }
        assert!(saw_endpoint);
        assert!(saw_final);
        // Continuous mode resumes Listening — no SessionCompleted yet.
        assert!(!saw_completed);
    }

    #[tokio::test]
    async fn in_process_rejects_sequence_gap() {
        let provider = Arc::new(NullSpeechProvider::new());
        let mut service = InProcessVoiceService::new(provider);
        let (events, mut rx) = SpeechEventSender::pair();

        service
            .handle_request(
                VoiceRequest::StartVoiceSession(StartVoiceSessionRequest {
                    config: SpeechSessionConfig {
                        session_id: "voice_1".into(),
                        language: None,
                        context: SessionContext {
                            document_id: None,
                            glossary_terms: Vec::new(),
                            known_paths: Vec::new(),
                            command_mode: false,
                        },
                        endpoint: EndpointOptions::default(),
                    },
                }),
                &events,
            )
            .await
            .unwrap();

        let _ = rx.recv().await;

        let err = service
            .handle_request(
                VoiceRequest::PushAudioChunk(sample_chunk("voice_1", 1)),
                &events,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, SpeechError::SequenceGap { .. }));
    }

    #[tokio::test]
    async fn update_session_context_stores_glossary_on_session() {
        let provider = Arc::new(NullSpeechProvider::new());
        let mut service = InProcessVoiceService::new(provider);
        let (events, mut rx) = SpeechEventSender::pair();

        service
            .handle_request(
                VoiceRequest::StartVoiceSession(StartVoiceSessionRequest {
                    config: SpeechSessionConfig {
                        session_id: "voice_1".into(),
                        language: None,
                        context: SessionContext {
                            document_id: Some("doc-1".into()),
                            glossary_terms: vec!["Home".into()],
                            known_paths: Vec::new(),
                            command_mode: false,
                        },
                        endpoint: EndpointOptions::default(),
                    },
                }),
                &events,
            )
            .await
            .unwrap();
        let _ = rx.recv().await;

        service
            .handle_request(
                VoiceRequest::UpdateSessionContext(UpdateSessionContextRequest {
                    session_id: "voice_1".into(),
                    context: SessionContext {
                        document_id: Some("doc-1".into()),
                        glossary_terms: vec!["Quick Note".into(), "Lattice".into()],
                        known_paths: Vec::new(),
                        command_mode: false,
                    },
                }),
                &events,
            )
            .await
            .unwrap();

        let context = service.session_context(&"voice_1".into()).unwrap();
        assert_eq!(
            context.glossary_terms,
            vec![String::from("Quick Note"), String::from("Lattice")]
        );
    }

    #[test]
    fn revision_ordering_rejects_regression() {
        let err = record_transcript_revision(&"voice_1".into(), 5, 4).unwrap_err();
        assert!(matches!(err, SpeechError::RevisionOutOfOrder { .. }));
    }
}
