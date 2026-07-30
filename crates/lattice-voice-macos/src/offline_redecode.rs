//! Parakeet TDT v2 offline re-decode backend (ADR 0007).
//!
//! Calls FluidAudio `AsrManager` through the Swift bridge when linked.

use std::sync::{Arc, Mutex};

use lattice_voice::{
    FinalizationMode, FrozenUtteranceAudio, OfflineRedecodeBackend, SpeechError,
};

use crate::bridge::VoiceBridgeBackend;
use crate::ffi::LatticeVoiceEngine;

const TDT_SAMPLE_RATE_HZ: u32 = 16_000;

/// Offline re-decode via Parakeet TDT v2 (`parakeet-tdt-0.6b-v2-coreml`).
pub struct TdtOfflineRedecode {
    backend: Arc<dyn VoiceBridgeBackend>,
    engine: Arc<Mutex<Option<LatticeVoiceEngine>>>,
}

impl TdtOfflineRedecode {
    pub fn new(
        backend: Arc<dyn VoiceBridgeBackend>,
        engine: Arc<Mutex<Option<LatticeVoiceEngine>>>,
    ) -> Self {
        Self { backend, engine }
    }

    fn engine_handle(&self) -> Result<LatticeVoiceEngine, SpeechError> {
        let guard = self
            .engine
            .lock()
            .map_err(|_| SpeechError::provider("engine lock poisoned"))?;
        guard.ok_or_else(|| SpeechError::provider("engine is not created"))
    }
}

impl OfflineRedecodeBackend for TdtOfflineRedecode {
    fn is_implemented(&self) -> bool {
        self.backend.offline_redecode_implemented()
    }

    fn finalization_mode(&self) -> FinalizationMode {
        FinalizationMode::IndependentOfflineRedecode
    }

    fn redecode(&self, audio: &FrozenUtteranceAudio) -> Result<String, SpeechError> {
        if audio.is_empty() {
            return Err(SpeechError::provider(
                "offline re-decode requires non-empty utterance audio",
            ));
        }
        if audio.sample_rate_hz() != TDT_SAMPLE_RATE_HZ {
            return Err(SpeechError::provider(format!(
                "offline re-decode requires {TDT_SAMPLE_RATE_HZ} Hz audio (got {})",
                audio.sample_rate_hz()
            )));
        }
        if audio.channels() != 1 {
            return Err(SpeechError::provider(format!(
                "offline re-decode requires mono audio (got {} channels)",
                audio.channels()
            )));
        }

        let engine = self.engine_handle()?;
        self.backend
            .engine_redecode_offline(engine, audio.samples(), audio.sample_rate_hz())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::MockBridge;
    use crate::LATTICE_VOICE_BRIDGE_ABI_VERSION;

    #[test]
    fn mock_backend_reports_not_implemented() {
        let bridge = Arc::new(MockBridge::new(LATTICE_VOICE_BRIDGE_ABI_VERSION));
        let engine = Arc::new(Mutex::new(Some(1)));
        let backend = TdtOfflineRedecode::new(bridge, engine);
        assert!(!backend.is_implemented());
        assert_eq!(
            backend.finalization_mode(),
            FinalizationMode::IndependentOfflineRedecode
        );
    }
}
