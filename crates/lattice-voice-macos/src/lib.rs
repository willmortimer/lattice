//! Thin Rust stub for the macOS FluidAudio bridge.
//!
//! The production ASR path is Parakeet Unified via Swift
//! `StreamingUnifiedAsrManager` (FluidAudio 0.15.5). The C ABI lives in
//! `swift/Sources/LatticeVoiceBridge` and `include/lattice_voice_bridge.h`.
//!
//! Full `SpeechProvider` implementation is Task R.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

/// ABI version expected from `lattice_voice_bridge_abi_version()`.
///
/// Must match `LATTICE_VOICE_BRIDGE_ABI_VERSION` in
/// `include/lattice_voice_bridge.h` and the Swift export.
pub const LATTICE_VOICE_BRIDGE_ABI_VERSION: u32 = 1;

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
