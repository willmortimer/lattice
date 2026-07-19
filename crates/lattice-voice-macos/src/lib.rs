//! macOS FluidAudio C ABI bridge and `SpeechProvider` for Lattice voice.
//!
//! The production ASR path is Parakeet Unified via Swift
//! `StreamingUnifiedAsrManager` (FluidAudio 0.15.5). The C ABI lives in
//! `swift/Sources/LatticeVoiceBridge` and `include/lattice_voice_bridge.h`.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod bridge;
mod error;
mod ffi;
mod provider;

/// ABI version expected from `lattice_voice_bridge_abi_version()`.
///
/// Must match `LATTICE_VOICE_BRIDGE_ABI_VERSION` in
/// `include/lattice_voice_bridge.h` and the Swift export.
pub const LATTICE_VOICE_BRIDGE_ABI_VERSION: u32 = 1;

pub use error::ensure_abi_version;
pub use provider::{default_model_cache_dir, FluidAudioSpeechProvider};

/// Returns the ABI version this crate expects from the native bridge.
#[must_use]
pub fn expected_bridge_abi_version() -> u32 {
    LATTICE_VOICE_BRIDGE_ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(LATTICE_VOICE_BRIDGE_ABI_VERSION, 1);
        assert_eq!(expected_bridge_abi_version(), 1);
    }
}
