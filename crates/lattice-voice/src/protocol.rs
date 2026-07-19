use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Current daemon-compatible protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

pub type VoiceSessionId = String;
pub type UtteranceId = String;
pub type LanguageTag = String;

/// Canonical PCM sample encoding on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSampleFormat {
    F32,
    I16Le,
}

/// Monotonic PCM chunk from a trusted local capture client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioChunk {
    pub session_id: VoiceSessionId,
    pub sequence: u64,
    pub captured_at_ns: u64,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub sample_format: AudioSampleFormat,
    #[serde(with = "serde_bytes")]
    pub payload: Bytes,
}

/// How a provider produces authoritative finals (voice ADR 0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalizationMode {
    /// Authoritative flush of the streaming checkpoint (Unified `finish()`).
    StreamingFlush,
    /// Separate offline encoder in the same model family re-decodes utterance audio.
    SameFamilyOfflineRedecode,
    /// Distinct final model re-decodes the full utterance.
    IndependentOfflineRedecode,
}

/// Provider capability negotiation surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechCapabilities {
    pub streaming: bool,
    pub partial_transcripts: bool,
    pub finalization_mode: FinalizationMode,
    pub punctuation: bool,
    pub word_timestamps: bool,
    pub language_detection: bool,
    pub vocabulary_biasing: bool,
    pub endpoint_detection: bool,
    pub supported_languages: Vec<LanguageTag>,
}

/// Session lifecycle states from the transcription pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionSessionState {
    Created,
    Preparing,
    Ready,
    Listening,
    SpeechActive,
    Finalizing,
    Completed,
    Cancelled,
    Failed,
}

/// Model preparation lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
    Unavailable,
    Downloading,
    Verifying,
    Preparing,
    Ready,
    Unloading,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelStatus {
    pub state: ModelState,
    pub model_version: Option<String>,
    pub provider_version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareModelRequest {
    pub model_id: String,
    pub warm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionContext {
    pub document_id: Option<String>,
    pub glossary_terms: Vec<String>,
    pub command_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechSessionConfig {
    pub session_id: VoiceSessionId,
    pub language: Option<LanguageTag>,
    pub context: SessionContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartVoiceSessionRequest {
    pub config: SpeechSessionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishUtteranceRequest {
    pub session_id: VoiceSessionId,
    pub utterance_id: UtteranceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateSessionContextRequest {
    pub session_id: VoiceSessionId,
    pub context: SessionContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelVoiceSessionRequest {
    pub session_id: VoiceSessionId,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndVoiceSessionRequest {
    pub session_id: VoiceSessionId,
}

/// Client → voice service request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceRequest {
    PrepareModel(PrepareModelRequest),
    GetVoiceCapabilities,
    StartVoiceSession(StartVoiceSessionRequest),
    PushAudioChunk(AudioChunk),
    FinishUtterance(FinishUtteranceRequest),
    UpdateSessionContext(UpdateSessionContextRequest),
    CancelVoiceSession(CancelVoiceSessionRequest),
    EndVoiceSession(EndVoiceSessionRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialTranscriptPayload {
    pub session_id: VoiceSessionId,
    pub utterance_id: UtteranceId,
    pub revision: u64,
    pub text: String,
    pub stable_prefix_bytes: u32,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableTranscriptPayload {
    pub session_id: VoiceSessionId,
    pub utterance_id: UtteranceId,
    pub revision: u64,
    pub text: String,
    pub stable_prefix_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalTranscript {
    pub session_id: VoiceSessionId,
    pub utterance_id: UtteranceId,
    pub replaces_revision: u64,
    pub text: String,
    pub finalization_mode: FinalizationMode,
    pub duration_ms: u64,
    pub processing_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandCandidatePayload {
    pub session_id: VoiceSessionId,
    pub utterance_id: UtteranceId,
    pub command_id: String,
    pub confidence: f32,
    pub raw_text: String,
}

/// Voice service → client event envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceEvent {
    ModelStatusChanged(ModelStatus),
    SessionReady {
        session_id: VoiceSessionId,
        protocol_version: u32,
        capabilities: SpeechCapabilities,
    },
    SpeechStarted {
        session_id: VoiceSessionId,
        utterance_id: UtteranceId,
        started_at_ms: u64,
    },
    PartialTranscript(PartialTranscriptPayload),
    StableTranscript(StableTranscriptPayload),
    FinalTranscript(FinalTranscript),
    CommandCandidate(CommandCandidatePayload),
    SessionCompleted {
        session_id: VoiceSessionId,
        state: TranscriptionSessionState,
    },
    SessionFailed {
        session_id: VoiceSessionId,
        message: String,
        state: TranscriptionSessionState,
    },
}

mod serde_bytes {
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Vec::<u8>::deserialize(deserializer)?;
        Ok(Bytes::from(value))
    }
}
