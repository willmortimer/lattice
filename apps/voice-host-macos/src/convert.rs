//! Convert between `lattice-protocol` wire types and `lattice-voice` domain types.

use bytes::Bytes;
use lattice_protocol::{
    event, AudioSampleFormat as ProtoAudioSampleFormat, CancelVoiceSessionRequest as ProtoCancel,
    EndVoiceSessionRequest as ProtoEnd, Event, FinalTranscript as ProtoFinal,
    FinalizationMode as ProtoFinalizationMode, FinishUtteranceRequest as ProtoFinish,
    ModelState as ProtoModelState, ModelStatus as ProtoModelStatus,
    PartialTranscript as ProtoPartial, PrepareModelRequest as ProtoPrepare,
    PushAudioChunkRequest as ProtoPush, SessionContext as ProtoSessionContext,
    SpeechCapabilities as ProtoCapabilities, SpeechSessionConfig as ProtoSessionConfig,
    StableTranscript as ProtoStable, StartVoiceSessionRequest as ProtoStart,
    TranscriptionSessionState as ProtoSessionState,
    UpdateSessionContextRequest as ProtoUpdateContext,
};
use lattice_voice::{
    AudioChunk, AudioSampleFormat, CancelVoiceSessionRequest, EndVoiceSessionRequest,
    FinalTranscript, FinalizationMode, FinishUtteranceRequest, ModelState, ModelStatus,
    PartialTranscriptPayload, PrepareModelRequest, SessionContext, SpeechCapabilities,
    SpeechSessionConfig, StableTranscriptPayload, StartVoiceSessionRequest,
    TranscriptionSessionState, UpdateSessionContextRequest, VoiceEvent, VoiceRequest,
};

use crate::error::VoiceHostError;

pub fn voice_request_from_proto(
    body: lattice_protocol::request::Body,
) -> Result<Option<VoiceRequest>, VoiceHostError> {
    use lattice_protocol::request::Body;
    match body {
        Body::PrepareModel(req) => Ok(Some(VoiceRequest::PrepareModel(prepare_from_proto(req)))),
        Body::GetVoiceCapabilities(_) => Ok(Some(VoiceRequest::GetVoiceCapabilities)),
        Body::StartVoiceSession(req) => Ok(Some(VoiceRequest::StartVoiceSession(
            start_from_proto(req)?,
        ))),
        Body::PushAudioChunk(req) => Ok(Some(VoiceRequest::PushAudioChunk(chunk_from_proto(
            req,
        )?))),
        Body::FinishUtterance(req) => Ok(Some(VoiceRequest::FinishUtterance(finish_from_proto(
            req,
        )))),
        Body::UpdateSessionContext(req) => Ok(Some(VoiceRequest::UpdateSessionContext(
            update_context_from_proto(req),
        ))),
        Body::CancelVoiceSession(req) => Ok(Some(VoiceRequest::CancelVoiceSession(
            cancel_from_proto(req),
        ))),
        Body::EndVoiceSession(req) => Ok(Some(VoiceRequest::EndVoiceSession(end_from_proto(req)))),
        Body::Health(_)
        | Body::Ping(_)
        | Body::OpenWorkspace(_)
        | Body::Search(_)
        | Body::ApplyPageUpdate(_)
        | Body::VoiceHostStatus(_)
        | Body::UnloadVoiceModel(_) => Ok(None),
    }
}

pub fn event_from_domain(sequence: u64, event: VoiceEvent) -> Result<Event, VoiceHostError> {
    let body = match event {
        VoiceEvent::ModelStatusChanged(status) => {
            event::Body::ModelStatus(lattice_protocol::ModelStatusChanged {
                status: Some(model_status_to_proto(status)),
            })
        }
        VoiceEvent::SessionReady {
            session_id,
            protocol_version,
            capabilities,
        } => event::Body::SessionReady(lattice_protocol::SessionReady {
            session_id,
            protocol_version,
            capabilities: Some(capabilities_to_proto(capabilities)),
        }),
        VoiceEvent::SpeechStarted {
            session_id,
            utterance_id,
            started_at_ms,
        } => event::Body::SpeechStarted(lattice_protocol::SpeechStarted {
            session_id,
            utterance_id,
            started_at_ms,
        }),
        VoiceEvent::EndpointDetected {
            session_id,
            utterance_id,
            ended_at_ms,
            reason,
        } => event::Body::EndpointDetected(lattice_protocol::EndpointDetected {
            session_id,
            utterance_id,
            ended_at_ms,
            reason: endpoint_reason_to_proto(reason).into(),
        }),
        VoiceEvent::PartialTranscript(partial) => {
            event::Body::PartialTranscript(partial_to_proto(partial))
        }
        VoiceEvent::StableTranscript(stable) => {
            event::Body::StableTranscript(stable_to_proto(stable))
        }
        VoiceEvent::FinalTranscript(final_transcript) => {
            event::Body::FinalTranscript(final_to_proto(final_transcript))
        }
        VoiceEvent::CommandCandidate(candidate) => {
            event::Body::CommandCandidate(lattice_protocol::CommandCandidate {
                session_id: candidate.session_id,
                utterance_id: candidate.utterance_id,
                command_id: candidate.command_id,
                confidence: candidate.confidence,
                raw_text: candidate.raw_text,
            })
        }
        VoiceEvent::SessionCompleted { session_id, state } => {
            event::Body::SessionCompleted(lattice_protocol::SessionCompleted {
                session_id,
                state: session_state_to_proto(state).into(),
            })
        }
        VoiceEvent::SessionFailed {
            session_id,
            message,
            state,
        } => event::Body::SessionFailed(lattice_protocol::SessionFailed {
            session_id,
            message,
            state: session_state_to_proto(state).into(),
        }),
    };

    Ok(Event {
        sequence,
        workspace_id: String::new(),
        body: Some(body),
    })
}

pub fn prepare_from_proto(req: ProtoPrepare) -> PrepareModelRequest {
    PrepareModelRequest {
        model_id: req.model_id,
        warm: req.warm,
    }
}

pub fn model_status_to_proto(status: ModelStatus) -> ProtoModelStatus {
    ProtoModelStatus {
        state: model_state_to_proto(status.state).into(),
        model_version: status.model_version,
        provider_version: status.provider_version,
        message: status.message,
    }
}

pub fn model_status_from_proto(status: ProtoModelStatus) -> Result<ModelStatus, VoiceHostError> {
    Ok(ModelStatus {
        state: model_state_from_proto(status.state())?,
        model_version: status.model_version,
        provider_version: status.provider_version,
        message: status.message,
    })
}

pub fn capabilities_to_proto(caps: SpeechCapabilities) -> ProtoCapabilities {
    ProtoCapabilities {
        streaming: caps.streaming,
        partial_transcripts: caps.partial_transcripts,
        finalization_mode: finalization_to_proto(caps.finalization_mode).into(),
        punctuation: caps.punctuation,
        word_timestamps: caps.word_timestamps,
        language_detection: caps.language_detection,
        vocabulary_biasing: caps.vocabulary_biasing,
        endpoint_detection: caps.endpoint_detection,
        supported_languages: caps.supported_languages,
    }
}

pub fn capabilities_from_proto(
    caps: ProtoCapabilities,
) -> Result<SpeechCapabilities, VoiceHostError> {
    Ok(SpeechCapabilities {
        streaming: caps.streaming,
        partial_transcripts: caps.partial_transcripts,
        finalization_mode: finalization_from_proto(caps.finalization_mode())?,
        punctuation: caps.punctuation,
        word_timestamps: caps.word_timestamps,
        language_detection: caps.language_detection,
        vocabulary_biasing: caps.vocabulary_biasing,
        endpoint_detection: caps.endpoint_detection,
        supported_languages: caps.supported_languages,
    })
}

fn start_from_proto(req: ProtoStart) -> Result<StartVoiceSessionRequest, VoiceHostError> {
    let config = req
        .config
        .ok_or_else(|| VoiceHostError::protocol("start_voice_session missing config"))?;
    Ok(StartVoiceSessionRequest {
        config: session_config_from_proto(config),
    })
}

fn session_config_from_proto(config: ProtoSessionConfig) -> SpeechSessionConfig {
    SpeechSessionConfig {
        session_id: config.session_id,
        language: config.language,
        context: config
            .context
            .map(session_context_from_proto)
            .unwrap_or(SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                known_paths: Vec::new(),
                command_mode: false,
            }),
        endpoint: config
            .endpoint
            .map(endpoint_options_from_proto)
            .unwrap_or_default(),
    }
}

fn endpoint_options_from_proto(
    options: lattice_protocol::EndpointOptions,
) -> lattice_voice::EndpointOptions {
    use lattice_voice::{DEFAULT_MAX_UTTERANCE_MS, DEFAULT_SILENCE_DEBOUNCE_MS};
    lattice_voice::EndpointOptions {
        auto_finalize_on_endpoint: options.auto_finalize_on_endpoint,
        silence_debounce_ms: if options.silence_debounce_ms == 0 {
            DEFAULT_SILENCE_DEBOUNCE_MS
        } else {
            options.silence_debounce_ms
        },
        max_utterance_ms: if options.max_utterance_ms == 0 {
            DEFAULT_MAX_UTTERANCE_MS
        } else {
            options.max_utterance_ms
        },
    }
}

fn endpoint_reason_to_proto(
    reason: lattice_voice::EndpointReason,
) -> lattice_protocol::EndpointReason {
    match reason {
        lattice_voice::EndpointReason::SilenceDebounce => {
            lattice_protocol::EndpointReason::SilenceDebounce
        }
        lattice_voice::EndpointReason::MaxUtteranceLength => {
            lattice_protocol::EndpointReason::MaxUtteranceLength
        }
        lattice_voice::EndpointReason::ProviderEou => {
            lattice_protocol::EndpointReason::ProviderEou
        }
    }
}

fn session_context_from_proto(context: ProtoSessionContext) -> SessionContext {
    SessionContext {
        document_id: context.document_id,
        glossary_terms: context.glossary_terms,
        known_paths: context.known_paths,
        command_mode: context.command_mode,
    }
}

fn chunk_from_proto(req: ProtoPush) -> Result<AudioChunk, VoiceHostError> {
    let channels = u8::try_from(req.channels).map_err(|_| {
        VoiceHostError::protocol(format!(
            "channels {} exceeds u8 (max 255)",
            req.channels
        ))
    })?;
    let sample_format = sample_format_from_proto(req.sample_format())?;
    Ok(AudioChunk {
        session_id: req.session_id,
        sequence: req.sequence,
        captured_at_ns: req.captured_at_ns,
        sample_rate_hz: req.sample_rate_hz,
        channels,
        sample_format,
        payload: Bytes::from(req.payload),
    })
}

fn finish_from_proto(req: ProtoFinish) -> FinishUtteranceRequest {
    FinishUtteranceRequest {
        session_id: req.session_id,
        utterance_id: req.utterance_id,
    }
}

fn update_context_from_proto(req: ProtoUpdateContext) -> UpdateSessionContextRequest {
    UpdateSessionContextRequest {
        session_id: req.session_id,
        context: req
            .context
            .map(session_context_from_proto)
            .unwrap_or(SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                known_paths: Vec::new(),
                command_mode: false,
            }),
    }
}

fn cancel_from_proto(req: ProtoCancel) -> CancelVoiceSessionRequest {
    CancelVoiceSessionRequest {
        session_id: req.session_id,
        reason: req.reason,
    }
}

fn end_from_proto(req: ProtoEnd) -> EndVoiceSessionRequest {
    EndVoiceSessionRequest {
        session_id: req.session_id,
    }
}

fn partial_to_proto(partial: PartialTranscriptPayload) -> ProtoPartial {
    ProtoPartial {
        session_id: partial.session_id,
        utterance_id: partial.utterance_id,
        revision: partial.revision,
        text: partial.text,
        stable_prefix_bytes: partial.stable_prefix_bytes,
        started_at_ms: partial.started_at_ms,
        ended_at_ms: partial.ended_at_ms,
    }
}

fn stable_to_proto(stable: StableTranscriptPayload) -> ProtoStable {
    ProtoStable {
        session_id: stable.session_id,
        utterance_id: stable.utterance_id,
        revision: stable.revision,
        text: stable.text,
        stable_prefix_bytes: stable.stable_prefix_bytes,
    }
}

fn final_to_proto(final_transcript: FinalTranscript) -> ProtoFinal {
    ProtoFinal {
        session_id: final_transcript.session_id,
        utterance_id: final_transcript.utterance_id,
        replaces_revision: final_transcript.replaces_revision,
        text: final_transcript.text,
        finalization_mode: finalization_to_proto(final_transcript.finalization_mode).into(),
        duration_ms: final_transcript.duration_ms,
        processing_ms: final_transcript.processing_ms,
    }
}

fn sample_format_from_proto(
    value: ProtoAudioSampleFormat,
) -> Result<AudioSampleFormat, VoiceHostError> {
    match value {
        ProtoAudioSampleFormat::F32 => Ok(AudioSampleFormat::F32),
        ProtoAudioSampleFormat::I16Le => Ok(AudioSampleFormat::I16Le),
        ProtoAudioSampleFormat::Unspecified => Err(VoiceHostError::protocol(
            "audio sample format is unspecified",
        )),
    }
}

fn finalization_to_proto(mode: FinalizationMode) -> ProtoFinalizationMode {
    match mode {
        FinalizationMode::StreamingFlush => ProtoFinalizationMode::StreamingFlush,
        FinalizationMode::SameFamilyOfflineRedecode => {
            ProtoFinalizationMode::SameFamilyOfflineRedecode
        }
        FinalizationMode::IndependentOfflineRedecode => {
            ProtoFinalizationMode::IndependentOfflineRedecode
        }
    }
}

fn finalization_from_proto(
    mode: ProtoFinalizationMode,
) -> Result<FinalizationMode, VoiceHostError> {
    match mode {
        ProtoFinalizationMode::StreamingFlush => Ok(FinalizationMode::StreamingFlush),
        ProtoFinalizationMode::SameFamilyOfflineRedecode => {
            Ok(FinalizationMode::SameFamilyOfflineRedecode)
        }
        ProtoFinalizationMode::IndependentOfflineRedecode => {
            Ok(FinalizationMode::IndependentOfflineRedecode)
        }
        ProtoFinalizationMode::Unspecified => Err(VoiceHostError::protocol(
            "finalization mode is unspecified",
        )),
    }
}

fn model_state_to_proto(state: ModelState) -> ProtoModelState {
    match state {
        ModelState::Unavailable => ProtoModelState::Unavailable,
        ModelState::Downloading => ProtoModelState::Downloading,
        ModelState::Verifying => ProtoModelState::Verifying,
        ModelState::Preparing => ProtoModelState::Preparing,
        ModelState::Ready => ProtoModelState::Ready,
        ModelState::Unloading => ProtoModelState::Unloading,
        ModelState::Failed => ProtoModelState::Failed,
    }
}

fn model_state_from_proto(state: ProtoModelState) -> Result<ModelState, VoiceHostError> {
    match state {
        ProtoModelState::Unavailable => Ok(ModelState::Unavailable),
        ProtoModelState::Downloading => Ok(ModelState::Downloading),
        ProtoModelState::Verifying => Ok(ModelState::Verifying),
        ProtoModelState::Preparing => Ok(ModelState::Preparing),
        ProtoModelState::Ready => Ok(ModelState::Ready),
        ProtoModelState::Unloading => Ok(ModelState::Unloading),
        ProtoModelState::Failed => Ok(ModelState::Failed),
        ProtoModelState::Unspecified => Err(VoiceHostError::protocol("model state is unspecified")),
    }
}

fn session_state_to_proto(state: TranscriptionSessionState) -> ProtoSessionState {
    match state {
        TranscriptionSessionState::Created => ProtoSessionState::Created,
        TranscriptionSessionState::Preparing => ProtoSessionState::Preparing,
        TranscriptionSessionState::Ready => ProtoSessionState::Ready,
        TranscriptionSessionState::Listening => ProtoSessionState::Listening,
        TranscriptionSessionState::SpeechActive => ProtoSessionState::SpeechActive,
        TranscriptionSessionState::Finalizing => ProtoSessionState::Finalizing,
        TranscriptionSessionState::Completed => ProtoSessionState::Completed,
        TranscriptionSessionState::Cancelled => ProtoSessionState::Cancelled,
        TranscriptionSessionState::Failed => ProtoSessionState::Failed,
    }
}
