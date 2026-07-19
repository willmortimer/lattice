//! Energy-based endpoint policy for continuous dictation.
//!
//! FluidAudio Unified (`StreamingUnifiedAsrManager`) does not expose
//! `setEouCallback` / `eouDebounceMs` (EOU-only API). Lattice therefore owns
//! speech-onset / silence-debounce / max-utterance decisions and maps them to
//! `SpeechStarted` / `EndpointDetected` events. Hold-to-talk ignores auto-finalize
//! and still finalizes via explicit `FinishUtterance`.

use crate::protocol::EndpointReason;

/// Default silence required before an utterance endpoint (transcription-pipeline).
pub const DEFAULT_SILENCE_DEBOUNCE_MS: u32 = 800;

/// Default maximum utterance duration before a forced endpoint.
pub const DEFAULT_MAX_UTTERANCE_MS: u32 = 45_000;

/// Env flag: when `1`/`true`, continuous sessions auto-finalize on endpoint
/// even if the session option is unset.
pub const ENV_AUTO_FINALIZE_ON_ENDPOINT: &str = "LATTICE_VOICE_AUTO_FINALIZE_ON_ENDPOINT";

/// Session-level endpoint / continuous-dictation knobs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EndpointOptions {
    /// When true, silence debounce / max length auto-invoke utterance finalization.
    /// Hold-to-talk leaves this false and calls `FinishUtterance` on release.
    #[serde(default)]
    pub auto_finalize_on_endpoint: bool,
    /// Silence duration (ms) after speech before an endpoint.
    #[serde(default = "default_silence_debounce_ms")]
    pub silence_debounce_ms: u32,
    /// Hard cap on utterance length (ms).
    #[serde(default = "default_max_utterance_ms")]
    pub max_utterance_ms: u32,
}

fn default_silence_debounce_ms() -> u32 {
    DEFAULT_SILENCE_DEBOUNCE_MS
}

fn default_max_utterance_ms() -> u32 {
    DEFAULT_MAX_UTTERANCE_MS
}

impl Default for EndpointOptions {
    fn default() -> Self {
        Self {
            auto_finalize_on_endpoint: false,
            silence_debounce_ms: DEFAULT_SILENCE_DEBOUNCE_MS,
            max_utterance_ms: DEFAULT_MAX_UTTERANCE_MS,
        }
    }
}

impl EndpointOptions {
    /// Resolve auto-finalize from session option or process env.
    pub fn auto_finalize_enabled(&self) -> bool {
        self.auto_finalize_on_endpoint || env_auto_finalize_enabled()
    }
}

/// True when `LATTICE_VOICE_AUTO_FINALIZE_ON_ENDPOINT` is set to a truthy value.
pub fn env_auto_finalize_enabled() -> bool {
    match std::env::var(ENV_AUTO_FINALIZE_ON_ENDPOINT) {
        Ok(value) => {
            let trimmed = value.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        }
        Err(_) => false,
    }
}

/// Signals produced while feeding PCM into [`EndpointPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSignal {
    None,
    SpeechStarted,
    Endpoint(EndpointReason),
}

/// Streaming energy VAD + silence debounce + max utterance length.
#[derive(Debug, Clone)]
pub struct EndpointPolicy {
    silence_debounce_ms: u32,
    max_utterance_ms: u32,
    /// RMS threshold above which a frame counts as speech.
    speech_rms_threshold: f32,
    speech_active: bool,
    /// Elapsed speech time in the current utterance (ms).
    utterance_elapsed_ms: u64,
    /// Consecutive silence while speech-active (ms).
    silence_elapsed_ms: u64,
}

impl EndpointPolicy {
    pub fn new(options: &EndpointOptions) -> Self {
        Self {
            silence_debounce_ms: options.silence_debounce_ms.max(1),
            max_utterance_ms: options.max_utterance_ms.max(1),
            speech_rms_threshold: 0.015,
            speech_active: false,
            utterance_elapsed_ms: 0,
            silence_elapsed_ms: 0,
        }
    }

    pub fn reset(&mut self) {
        self.speech_active = false;
        self.utterance_elapsed_ms = 0;
        self.silence_elapsed_ms = 0;
    }

    pub fn speech_active(&self) -> bool {
        self.speech_active
    }

    /// Feed mono Float32 samples at `sample_rate_hz`. Returns at most one signal.
    pub fn push_samples(&mut self, samples: &[f32], sample_rate_hz: u32) -> EndpointSignal {
        if samples.is_empty() || sample_rate_hz == 0 {
            return EndpointSignal::None;
        }

        let duration_ms =
            (samples.len() as u64).saturating_mul(1000) / u64::from(sample_rate_hz);
        let rms = rms(samples);
        let is_speech = rms >= self.speech_rms_threshold;

        if !self.speech_active {
            if is_speech {
                self.speech_active = true;
                self.utterance_elapsed_ms = duration_ms;
                self.silence_elapsed_ms = 0;
                return EndpointSignal::SpeechStarted;
            }
            return EndpointSignal::None;
        }

        self.utterance_elapsed_ms = self.utterance_elapsed_ms.saturating_add(duration_ms);

        if self.utterance_elapsed_ms >= u64::from(self.max_utterance_ms) {
            self.reset();
            return EndpointSignal::Endpoint(EndpointReason::MaxUtteranceLength);
        }

        if is_speech {
            self.silence_elapsed_ms = 0;
            return EndpointSignal::None;
        }

        self.silence_elapsed_ms = self.silence_elapsed_ms.saturating_add(duration_ms);
        if self.silence_elapsed_ms >= u64::from(self.silence_debounce_ms) {
            self.reset();
            return EndpointSignal::Endpoint(EndpointReason::SilenceDebounce);
        }

        EndpointSignal::None
    }
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Decode little-endian Float32 mono PCM from an audio payload.
pub fn decode_f32_le(payload: &[u8]) -> Vec<f32> {
    let mut samples = Vec::with_capacity(payload.len() / 4);
    for bytes in payload.chunks_exact(4) {
        samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speech_chunk(ms: u32, sample_rate: u32, amplitude: f32) -> Vec<f32> {
        let n = (u64::from(ms) * u64::from(sample_rate) / 1000) as usize;
        vec![amplitude; n]
    }

    #[test]
    fn silence_never_starts_speech() {
        let mut policy = EndpointPolicy::new(&EndpointOptions::default());
        let samples = speech_chunk(200, 16_000, 0.0);
        assert_eq!(
            policy.push_samples(&samples, 16_000),
            EndpointSignal::None
        );
    }

    #[test]
    fn speech_then_silence_emits_endpoint() {
        let options = EndpointOptions {
            silence_debounce_ms: 100,
            ..EndpointOptions::default()
        };
        let mut policy = EndpointPolicy::new(&options);

        assert_eq!(
            policy.push_samples(&speech_chunk(50, 16_000, 0.2), 16_000),
            EndpointSignal::SpeechStarted
        );
        assert_eq!(
            policy.push_samples(&speech_chunk(50, 16_000, 0.2), 16_000),
            EndpointSignal::None
        );
        assert_eq!(
            policy.push_samples(&speech_chunk(120, 16_000, 0.0), 16_000),
            EndpointSignal::Endpoint(EndpointReason::SilenceDebounce)
        );
        assert!(!policy.speech_active());
    }

    #[test]
    fn max_utterance_forces_endpoint() {
        let options = EndpointOptions {
            max_utterance_ms: 80,
            silence_debounce_ms: 10_000,
            ..EndpointOptions::default()
        };
        let mut policy = EndpointPolicy::new(&options);
        assert_eq!(
            policy.push_samples(&speech_chunk(50, 16_000, 0.2), 16_000),
            EndpointSignal::SpeechStarted
        );
        assert_eq!(
            policy.push_samples(&speech_chunk(50, 16_000, 0.2), 16_000),
            EndpointSignal::Endpoint(EndpointReason::MaxUtteranceLength)
        );
    }

    #[test]
    fn hold_to_talk_defaults_disable_auto_finalize() {
        assert!(!EndpointOptions::default().auto_finalize_enabled());
    }
}
