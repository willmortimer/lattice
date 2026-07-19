//! Optional independent / same-family offline final path (ADR 0007).
//!
//! Production default remains [`FinalizationMode::StreamingFlush`]. An optional
//! second decode runs only when explicitly enabled **and** a real offline
//! backend is implemented. Stubs must not claim offline modes.

use crate::error::SpeechError;
use crate::protocol::{FinalTranscript, FinalizationMode, SpeechCapabilities};
use crate::utterance_buffer::FrozenUtteranceAudio;

/// Env flag to attempt an independent final when a backend is implemented.
///
/// Value must be exactly `1`. Does not download models by itself; production
/// still commits [`FinalizationMode::StreamingFlush`] when the backend is a stub.
pub const ENV_INDEPENDENT_FINAL: &str = "LATTICE_VOICE_INDEPENDENT_FINAL";

/// Whether `LATTICE_VOICE_INDEPENDENT_FINAL=1` is set in the process environment.
#[must_use]
pub fn independent_final_env_enabled() -> bool {
    std::env::var(ENV_INDEPENDENT_FINAL)
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

/// Policy for attempting a second decode after streaming flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndependentFinalPolicy {
    pub env_enabled: bool,
    pub capability_allows: bool,
}

impl IndependentFinalPolicy {
    #[must_use]
    pub fn from_env_and_capabilities(capabilities: &SpeechCapabilities) -> Self {
        Self {
            env_enabled: independent_final_env_enabled(),
            capability_allows: capability_allows_offline_redecode(capabilities.finalization_mode),
        }
    }

    #[must_use]
    pub fn for_tests(env_enabled: bool, capability_allows: bool) -> Self {
        Self {
            env_enabled,
            capability_allows,
        }
    }

    #[must_use]
    pub fn should_attempt(&self) -> bool {
        self.env_enabled || self.capability_allows
    }
}

#[must_use]
pub fn capability_allows_offline_redecode(mode: FinalizationMode) -> bool {
    matches!(
        mode,
        FinalizationMode::SameFamilyOfflineRedecode
            | FinalizationMode::IndependentOfflineRedecode
    )
}

/// Result of optionally attempting an offline re-decode after streaming flush.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndependentFinalAttempt {
    /// Policy did not request a second path.
    Skipped {
        reason: &'static str,
    },
    /// Policy requested a second path, but no real backend is available.
    Unavailable {
        reason: String,
    },
    /// Offline re-decode produced the text that should be committed.
    Succeeded {
        text: String,
        mode: FinalizationMode,
    },
}

/// Backend that re-decodes a frozen utterance buffer.
///
/// Implementors must only return `true` from [`OfflineRedecodeBackend::is_implemented`]
/// when they actually run a second model/encoder over buffered PCM.
pub trait OfflineRedecodeBackend: Send + Sync {
    fn is_implemented(&self) -> bool;

    /// Mode reported only when [`OfflineRedecodeBackend::is_implemented`] is true.
    fn finalization_mode(&self) -> FinalizationMode;

    fn redecode(&self, audio: &FrozenUtteranceAudio) -> Result<String, SpeechError>;
}

/// Production stub: offline / TDT re-decode is not wired through the bridge yet.
///
/// TODO(voice-v11): Call FluidAudio `AsrManager` (TDT v2) or Unified offline
/// encoder from `lattice-voice-macos` once eval adopts IndependentOfflineRedecode.
/// Until then this backend never claims offline finalization modes.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnimplementedOfflineRedecode;

impl OfflineRedecodeBackend for UnimplementedOfflineRedecode {
    fn is_implemented(&self) -> bool {
        false
    }

    fn finalization_mode(&self) -> FinalizationMode {
        // Honest: stub does not perform IndependentOfflineRedecode.
        FinalizationMode::StreamingFlush
    }

    fn redecode(&self, _audio: &FrozenUtteranceAudio) -> Result<String, SpeechError> {
        Err(SpeechError::provider(
            "offline re-decode is not implemented (FluidAudio TDT / Unified offline TODO)",
        ))
    }
}

/// Test / harness backend that re-decodes by echoing buffer metadata.
#[derive(Debug, Clone)]
pub struct FakeIndependentOfflineRedecode {
    pub mode: FinalizationMode,
    pub text_prefix: String,
}

impl Default for FakeIndependentOfflineRedecode {
    fn default() -> Self {
        Self {
            mode: FinalizationMode::IndependentOfflineRedecode,
            text_prefix: "independent".into(),
        }
    }
}

impl OfflineRedecodeBackend for FakeIndependentOfflineRedecode {
    fn is_implemented(&self) -> bool {
        matches!(
            self.mode,
            FinalizationMode::SameFamilyOfflineRedecode
                | FinalizationMode::IndependentOfflineRedecode
        )
    }

    fn finalization_mode(&self) -> FinalizationMode {
        self.mode
    }

    fn redecode(&self, audio: &FrozenUtteranceAudio) -> Result<String, SpeechError> {
        if !self.is_implemented() {
            return Err(SpeechError::provider(
                "fake offline backend is not configured as implemented",
            ));
        }
        Ok(format!(
            "{}-frames-{}-samples-{}",
            self.text_prefix,
            audio.frames_accepted(),
            audio.len_samples()
        ))
    }
}

/// Attempt offline re-decode when policy allows and the backend is real.
pub fn attempt_independent_final(
    policy: IndependentFinalPolicy,
    backend: &dyn OfflineRedecodeBackend,
    audio: &FrozenUtteranceAudio,
) -> IndependentFinalAttempt {
    if !policy.should_attempt() {
        return IndependentFinalAttempt::Skipped {
            reason: "independent final disabled (default StreamingFlush)",
        };
    }
    if !backend.is_implemented() {
        return IndependentFinalAttempt::Unavailable {
            reason: format!(
                "{ENV_INDEPENDENT_FINAL} requested independent final, but offline backend is not implemented"
            ),
        };
    }

    match backend.redecode(audio) {
        Ok(text) => IndependentFinalAttempt::Succeeded {
            text,
            mode: backend.finalization_mode(),
        },
        Err(err) => IndependentFinalAttempt::Unavailable {
            reason: err.to_string(),
        },
    }
}

/// Choose the transcript that should be committed after streaming flush.
///
/// Always starts from the streaming-flush baseline. Replaces text/mode only when
/// an implemented offline backend succeeds.
#[must_use]
pub fn commit_final_transcript(
    mut streaming_flush: FinalTranscript,
    policy: IndependentFinalPolicy,
    backend: &dyn OfflineRedecodeBackend,
    audio: &FrozenUtteranceAudio,
) -> FinalTranscript {
    streaming_flush.finalization_mode = FinalizationMode::StreamingFlush;

    match attempt_independent_final(policy, backend, audio) {
        IndependentFinalAttempt::Succeeded { text, mode } => {
            streaming_flush.text = text;
            streaming_flush.finalization_mode = mode;
            streaming_flush
        }
        IndependentFinalAttempt::Skipped { .. }
        | IndependentFinalAttempt::Unavailable { .. } => streaming_flush,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utterance_buffer::UtteranceAudioBuffer;

    fn baseline_final() -> FinalTranscript {
        FinalTranscript {
            session_id: "s".into(),
            utterance_id: "u".into(),
            replaces_revision: 1,
            text: "streaming final".into(),
            raw_text: None,
            corrections: Vec::new(),
            finalization_mode: FinalizationMode::StreamingFlush,
            duration_ms: 10,
            processing_ms: 5,
        }
    }

    fn frozen_two_frames() -> FrozenUtteranceAudio {
        let mut buffer = UtteranceAudioBuffer::new();
        buffer.push_f32(&[0.0, 1.0], 16_000, 1).unwrap();
        buffer.push_f32(&[0.5], 16_000, 1).unwrap();
        buffer.freeze()
    }

    #[test]
    fn default_policy_keeps_streaming_flush() {
        let audio = frozen_two_frames();
        let committed = commit_final_transcript(
            baseline_final(),
            IndependentFinalPolicy::for_tests(false, false),
            &UnimplementedOfflineRedecode,
            &audio,
        );
        assert_eq!(committed.text, "streaming final");
        assert_eq!(
            committed.finalization_mode,
            FinalizationMode::StreamingFlush
        );
    }

    #[test]
    fn env_without_backend_does_not_claim_independent() {
        let audio = frozen_two_frames();
        let committed = commit_final_transcript(
            baseline_final(),
            IndependentFinalPolicy::for_tests(true, false),
            &UnimplementedOfflineRedecode,
            &audio,
        );
        assert_eq!(
            committed.finalization_mode,
            FinalizationMode::StreamingFlush
        );
        assert_eq!(committed.text, "streaming final");
    }

    #[test]
    fn implemented_backend_commits_independent_mode() {
        let audio = frozen_two_frames();
        let backend = FakeIndependentOfflineRedecode::default();
        let committed = commit_final_transcript(
            baseline_final(),
            IndependentFinalPolicy::for_tests(true, false),
            &backend,
            &audio,
        );
        assert_eq!(
            committed.finalization_mode,
            FinalizationMode::IndependentOfflineRedecode
        );
        assert_eq!(committed.text, "independent-frames-2-samples-3");
    }

    #[test]
    fn unimplemented_backend_never_reports_independent_mode() {
        let stub = UnimplementedOfflineRedecode;
        assert!(!stub.is_implemented());
        assert_eq!(
            stub.finalization_mode(),
            FinalizationMode::StreamingFlush
        );
    }
}
