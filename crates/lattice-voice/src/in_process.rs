use std::collections::HashMap;
use std::sync::Arc;

use crate::error::SpeechError;
use crate::protocol::{
    AudioChunk, FinishUtteranceRequest, PROTOCOL_VERSION, TranscriptionSessionState,
    VoiceEvent, VoiceRequest, VoiceSessionId,
};
use crate::provider::{SpeechEventSender, SpeechProvider, SpeechSession};
use crate::session::SessionStateMachine;

struct ActiveSession {
    state: SessionStateMachine,
    provider_session: Box<dyn SpeechSession>,
    next_sequence: u64,
    last_revision: u64,
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
            VoiceRequest::UpdateSessionContext(_request) => {
                // Context updates are accepted in the skeleton; provider hooks arrive in M1.
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
                next_sequence: 0,
                last_revision: 0,
            },
        );

        Ok(())
    }

    async fn push_audio(
        &mut self,
        chunk: AudioChunk,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        let session = self.session_mut(&chunk.session_id)?;

        if chunk.sequence != session.next_sequence {
            return Err(SpeechError::SequenceGap {
                session_id: chunk.session_id.clone(),
                expected: session.next_sequence,
                received: chunk.sequence,
            });
        }

        if session.state.state() == TranscriptionSessionState::Listening {
            session
                .state
                .transition(TranscriptionSessionState::SpeechActive)?;
            events.send(VoiceEvent::SpeechStarted {
                session_id: chunk.session_id.clone(),
                utterance_id: "utt_1".into(),
                started_at_ms: 0,
            })?;
        }

        session.provider_session.push_audio(chunk).await?;
        session.next_sequence += 1;

        Ok(())
    }

    async fn finish_utterance(
        &mut self,
        request: FinishUtteranceRequest,
        events: &SpeechEventSender,
    ) -> Result<(), SpeechError> {
        let session = self.session_mut(&request.session_id)?;
        session
            .state
            .transition(TranscriptionSessionState::Finalizing)?;

        let final_transcript = session.provider_session.finish_utterance().await?;
        validate_revision(
            &request.session_id,
            session.last_revision,
            final_transcript.replaces_revision,
        )?;
        session.last_revision = final_transcript.replaces_revision;

        events.send(VoiceEvent::FinalTranscript(final_transcript))?;
        session
            .state
            .transition(TranscriptionSessionState::Completed)?;
        events.send(VoiceEvent::SessionCompleted {
            session_id: request.session_id.clone(),
            state: session.state.state(),
        })?;

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
    use crate::protocol::{
        AudioSampleFormat, SessionContext, SpeechSessionConfig, StartVoiceSessionRequest,
    };
    use crate::provider::NullSpeechProvider;

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
    async fn in_process_partial_to_final_happy_path() {
        let provider = Arc::new(NullSpeechProvider::new());
        let mut service = InProcessVoiceService::new(provider);
        let (events, mut rx) = SpeechEventSender::pair();

        service
            .handle_request(
                VoiceRequest::StartVoiceSession(StartVoiceSessionRequest {
                    config: SpeechSessionConfig {
                        session_id: "voice_1".into(),
                        language: Some("en".into()),
                        context: SessionContext {
                            document_id: None,
                            glossary_terms: Vec::new(),
                            command_mode: false,
                        },
                    },
                }),
                &events,
            )
            .await
            .unwrap();

        let ready = rx.recv().await.unwrap();
        assert!(matches!(ready, VoiceEvent::SessionReady { .. }));

        service
            .handle_request(
                VoiceRequest::PushAudioChunk(sample_chunk("voice_1", 0)),
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
                            command_mode: false,
                        },
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

    #[test]
    fn revision_ordering_rejects_regression() {
        let err = record_transcript_revision(&"voice_1".into(), 5, 4).unwrap_err();
        assert!(matches!(err, SpeechError::RevisionOutOfOrder { .. }));
    }
}
