//! Daemon-compatible voice protocol types and in-process service for Lattice.
//!
//! This crate defines the shared request/event schema, provider traits, session
//! state machine, and an in-process dispatcher used before `latticed` IPC lands.

mod context_builder;
mod endpoint;
mod error;
mod final_model_memory;
mod in_process;
mod independent_final;
mod normalize;
mod protocol;
mod provider;
mod session;
mod utterance_buffer;

pub use context_builder::{
    BuiltVoiceContext, EmbeddingGlossaryHook, VoiceContextBuilder, VoiceContextInput,
    DEFAULT_MAX_GLOSSARY_TERMS, DEFAULT_MIN_GLOSSARY_TERMS,
};
pub use endpoint::{
    decode_f32_le, env_auto_finalize_enabled, EndpointOptions, EndpointPolicy, EndpointSignal,
    DEFAULT_MAX_UTTERANCE_MS, DEFAULT_SILENCE_DEBOUNCE_MS, ENV_AUTO_FINALIZE_ON_ENDPOINT,
};
pub use error::SpeechError;
pub use final_model_memory::{
    FinalModelLoadAction, FinalModelMemoryPolicy, FinalModelResidency,
};
pub use in_process::{record_transcript_revision, InProcessVoiceService};
pub use independent_final::{
    attempt_independent_final, capability_allows_offline_redecode, commit_final_transcript,
    independent_final_env_enabled, FakeIndependentOfflineRedecode, IndependentFinalAttempt,
    IndependentFinalPolicy, OfflineRedecodeBackend, UnimplementedOfflineRedecode,
    ENV_INDEPENDENT_FINAL,
};
pub use normalize::{
    normalize_final_transcript, normalize_transcript, CorrectionKind, CorrectionProvenance,
    CorrectionSource, NormalizationContext, NormalizedTranscript, NORMALIZER_VERSION,
};
pub use protocol::{
    AudioChunk, AudioSampleFormat, CancelVoiceSessionRequest, CommandCandidatePayload,
    EndVoiceSessionRequest, EndpointReason, FinalTranscript, FinalizationMode,
    FinishUtteranceRequest, LanguageTag, ModelState, ModelStatus, PartialTranscriptPayload,
    PrepareModelRequest, PROTOCOL_VERSION, SessionContext, SpeechCapabilities, SpeechSessionConfig,
    StableTranscriptPayload, StartVoiceSessionRequest, TranscriptionSessionState,
    UpdateSessionContextRequest, UtteranceId, VoiceEvent, VoiceRequest, VoiceSessionId,
};
pub use provider::{
    NullSpeechProvider, SpeechEventSender, SpeechProvider, SpeechSession,
};
pub use session::SessionStateMachine;
pub use utterance_buffer::{FrozenUtteranceAudio, UtteranceAudioBuffer};
