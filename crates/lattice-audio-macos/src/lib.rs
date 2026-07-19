//! macOS AVAudioEngine capture bridge for Lattice voice.
//!
//! Swift owns `AVAudioEngine` + `AVAudioConverter` behind a stable C ABI.
//! This crate wraps that ABI as a [`lattice_audio::CaptureProvider`].

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod bridge;
mod error;
mod ffi;
mod provider;

/// ABI version expected from `lattice_audio_bridge_abi_version()`.
pub const LATTICE_AUDIO_BRIDGE_ABI_VERSION: u32 = 1;

pub use error::ensure_abi_version;
pub use provider::MacOsCaptureProvider;

/// Returns the ABI version this crate expects from the native bridge.
#[must_use]
pub fn expected_bridge_abi_version() -> u32 {
    LATTICE_AUDIO_BRIDGE_ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(LATTICE_AUDIO_BRIDGE_ABI_VERSION, 1);
        assert_eq!(expected_bridge_abi_version(), 1);
    }
}
