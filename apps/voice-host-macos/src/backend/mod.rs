mod fake;
#[cfg(feature = "fluidaudio")]
mod fluidaudio;

use std::path::PathBuf;
use std::sync::Arc;

use lattice_voice::SpeechProvider;

use crate::error::VoiceHostError;

pub use fake::FakeBackend;

/// Runtime backend selected at host start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Deterministic [`lattice_voice::NullSpeechProvider`] (CI / default).
    Fake,
    /// FluidAudio / Parakeet via `lattice-voice-macos` (feature-gated).
    #[cfg(feature = "fluidaudio")]
    FluidAudio,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fake => "fake",
            #[cfg(feature = "fluidaudio")]
            Self::FluidAudio => "fluidaudio",
        }
    }

    pub fn parse(value: &str) -> Result<Self, VoiceHostError> {
        match value {
            "fake" | "null" => Ok(Self::Fake),
            "fluidaudio" | "fluid-audio" | "parakeet" => {
                #[cfg(feature = "fluidaudio")]
                {
                    Ok(Self::FluidAudio)
                }
                #[cfg(not(feature = "fluidaudio"))]
                {
                    Err(VoiceHostError::BackendUnavailable(
                        "fluidaudio feature is not enabled; rebuild with --features fluidaudio (see README)"
                            .into(),
                    ))
                }
            }
            other => Err(VoiceHostError::protocol(format!(
                "unknown backend '{other}' (expected fake{})",
                if cfg!(feature = "fluidaudio") {
                    " or fluidaudio"
                } else {
                    ""
                }
            ))),
        }
    }
}

/// Construct a [`SpeechProvider`] for the selected backend.
pub fn open_provider(
    kind: BackendKind,
    model_cache_dir: Option<PathBuf>,
) -> Result<Arc<dyn SpeechProvider>, VoiceHostError> {
    match kind {
        BackendKind::Fake => {
            let _ = model_cache_dir;
            Ok(FakeBackend::new())
        }
        #[cfg(feature = "fluidaudio")]
        BackendKind::FluidAudio => fluidaudio::open(model_cache_dir),
    }
}
