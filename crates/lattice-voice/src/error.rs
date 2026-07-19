use crate::protocol::{TranscriptionSessionState, VoiceSessionId};

/// Errors produced by the voice protocol and in-process service.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SpeechError {
    /// The requested session does not exist.
    #[error("voice session not found: {session_id}")]
    SessionNotFound { session_id: VoiceSessionId },

    /// The session is already in a terminal state.
    #[error("voice session {session_id} is terminal in state {state:?}")]
    SessionTerminal {
        session_id: VoiceSessionId,
        state: TranscriptionSessionState,
    },

    /// The requested state transition is not allowed.
    #[error("invalid session state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: TranscriptionSessionState,
        to: TranscriptionSessionState,
    },

    /// An audio chunk arrived with a non-contiguous sequence number.
    #[error(
        "audio sequence gap for session {session_id}: expected {expected}, received {received}"
    )]
    SequenceGap {
        session_id: VoiceSessionId,
        expected: u64,
        received: u64,
    },

    /// A transcript revision regressed or duplicated.
    #[error(
        "transcript revision out of order for session {session_id}: last {last_revision}, received {received_revision}"
    )]
    RevisionOutOfOrder {
        session_id: VoiceSessionId,
        last_revision: u64,
        received_revision: u64,
    },

    /// The provider or session rejected the operation.
    #[error("provider error: {message}")]
    Provider { message: String },

    /// Event delivery failed because the subscriber disconnected.
    #[error("event subscriber disconnected")]
    EventSubscriberDisconnected,
}

impl SpeechError {
    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider {
            message: message.into(),
        }
    }
}
