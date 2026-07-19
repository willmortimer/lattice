//! Optional FluidAudio backend (feature `fluidaudio`).
//!
//! Links `lattice-voice-macos` with the Swift bridge. CI defaults to `fake`
//! so Parakeet models are never required for `cargo test`.

use std::path::PathBuf;
use std::sync::Arc;

use lattice_voice::SpeechProvider;
use lattice_voice_macos::FluidAudioSpeechProvider;

use crate::error::VoiceHostError;

pub fn open(model_cache_dir: Option<PathBuf>) -> Result<Arc<dyn SpeechProvider>, VoiceHostError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = model_cache_dir;
        return Err(VoiceHostError::BackendUnavailable(
            "fluidaudio backend is only available on macOS".into(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let provider = match model_cache_dir {
            Some(dir) => FluidAudioSpeechProvider::with_model_cache(dir),
            None => FluidAudioSpeechProvider::new(),
        }
        .map_err(|error| VoiceHostError::BackendUnavailable(error.to_string()))?;
        Ok(Arc::new(provider))
    }
}
