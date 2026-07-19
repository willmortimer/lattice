use crate::error::SpeechError;
use crate::protocol::{TranscriptionSessionState, VoiceSessionId};

/// Enforces valid transcription session state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStateMachine {
    session_id: VoiceSessionId,
    state: TranscriptionSessionState,
}

impl SessionStateMachine {
    pub fn new(session_id: VoiceSessionId) -> Self {
        Self {
            session_id,
            state: TranscriptionSessionState::Created,
        }
    }

    pub fn session_id(&self) -> &VoiceSessionId {
        &self.session_id
    }

    pub fn state(&self) -> TranscriptionSessionState {
        self.state
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TranscriptionSessionState::Completed
                | TranscriptionSessionState::Cancelled
                | TranscriptionSessionState::Failed
        )
    }

    pub fn transition(&mut self, next: TranscriptionSessionState) -> Result<(), SpeechError> {
        if self.is_terminal() {
            return Err(SpeechError::SessionTerminal {
                session_id: self.session_id.clone(),
                state: self.state,
            });
        }

        if !is_valid_transition(self.state, next) {
            return Err(SpeechError::InvalidStateTransition {
                from: self.state,
                to: next,
            });
        }

        self.state = next;
        Ok(())
    }

    pub fn fail(&mut self) -> Result<(), SpeechError> {
        self.transition(TranscriptionSessionState::Failed)
    }

    pub fn cancel(&mut self) -> Result<(), SpeechError> {
        self.transition(TranscriptionSessionState::Cancelled)
    }
}

fn is_valid_transition(
    from: TranscriptionSessionState,
    to: TranscriptionSessionState,
) -> bool {
    use TranscriptionSessionState::*;

    if from == to {
        return false;
    }

    match (from, to) {
        (Created, Preparing) => true,
        (Preparing, Ready) => true,
        (Ready, Listening) => true,
        (Listening, SpeechActive) => true,
        (SpeechActive, Finalizing) => true,
        // Continuous dictation: finalize one utterance, then accept the next.
        (Finalizing, Listening) => true,
        (Finalizing, Completed) => true,
        (Created | Preparing | Ready | Listening | SpeechActive | Finalizing, Cancelled) => true,
        (Created | Preparing | Ready | Listening | SpeechActive | Finalizing, Failed) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions() {
        let mut machine = SessionStateMachine::new("voice_1".into());
        assert_eq!(machine.state(), TranscriptionSessionState::Created);

        machine
            .transition(TranscriptionSessionState::Preparing)
            .unwrap();
        machine.transition(TranscriptionSessionState::Ready).unwrap();
        machine
            .transition(TranscriptionSessionState::Listening)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::SpeechActive)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::Finalizing)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::Completed)
            .unwrap();
        assert!(machine.is_terminal());
    }

    #[test]
    fn invalid_transition_is_rejected_without_mutation() {
        let mut machine = SessionStateMachine::new("voice_1".into());
        let err = machine
            .transition(TranscriptionSessionState::Listening)
            .unwrap_err();
        assert_eq!(
            err,
            SpeechError::InvalidStateTransition {
                from: TranscriptionSessionState::Created,
                to: TranscriptionSessionState::Listening,
            }
        );
        assert_eq!(machine.state(), TranscriptionSessionState::Created);
    }

    #[test]
    fn active_state_may_cancel() {
        let mut machine = SessionStateMachine::new("voice_1".into());
        machine
            .transition(TranscriptionSessionState::Preparing)
            .unwrap();
        machine.cancel().unwrap();
        assert_eq!(machine.state(), TranscriptionSessionState::Cancelled);
    }

    #[test]
    fn terminal_state_rejects_further_transitions() {
        let mut machine = SessionStateMachine::new("voice_1".into());
        machine
            .transition(TranscriptionSessionState::Preparing)
            .unwrap();
        machine.transition(TranscriptionSessionState::Ready).unwrap();
        machine
            .transition(TranscriptionSessionState::Listening)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::SpeechActive)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::Finalizing)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::Completed)
            .unwrap();

        let err = machine
            .transition(TranscriptionSessionState::Failed)
            .unwrap_err();
        assert!(matches!(err, SpeechError::SessionTerminal { .. }));
    }

    #[test]
    fn continuous_resume_after_finalize() {
        let mut machine = SessionStateMachine::new("voice_1".into());
        machine
            .transition(TranscriptionSessionState::Preparing)
            .unwrap();
        machine.transition(TranscriptionSessionState::Ready).unwrap();
        machine
            .transition(TranscriptionSessionState::Listening)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::SpeechActive)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::Finalizing)
            .unwrap();
        machine
            .transition(TranscriptionSessionState::Listening)
            .unwrap();
        assert_eq!(machine.state(), TranscriptionSessionState::Listening);
    }
}
