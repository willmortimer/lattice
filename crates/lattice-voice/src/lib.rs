//! Daemon-compatible voice protocol types and in-process service for Lattice.
//!
//! This crate defines the shared request/event schema, provider traits, session
//! state machine, and an in-process dispatcher used before `latticed` IPC lands.

mod error;
mod in_process;
mod protocol;
mod provider;
mod session;

pub use error::SpeechError;
pub use in_process::{record_transcript_revision, InProcessVoiceService};
pub use protocol::{
    AudioChunk, AudioSampleFormat, CancelVoiceSessionRequest, CommandCandidatePayload,
    EndVoiceSessionRequest, FinalTranscript, FinalizationMode, FinishUtteranceRequest,
    LanguageTag,
    ModelState, ModelStatus, PartialTranscriptPayload, PrepareModelRequest, PROTOCOL_VERSION,
    SessionContext, SpeechCapabilities, SpeechSessionConfig, StableTranscriptPayload,
    StartVoiceSessionRequest, TranscriptionSessionState, UpdateSessionContextRequest, UtteranceId,
    VoiceEvent, VoiceRequest, VoiceSessionId,
};
pub use provider::{
    NullSpeechProvider, SpeechEventSender, SpeechProvider, SpeechSession,
};
pub use session::SessionStateMachine;
