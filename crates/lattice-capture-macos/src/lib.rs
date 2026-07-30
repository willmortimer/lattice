//! macOS ScreenCaptureKit capture bridge for Lattice clipper.
//!
//! Swift owns ScreenCaptureKit + AppKit overlay scaffolding behind a stable C ABI.
//! This crate wraps that ABI as a [`lattice_capture_core::CaptureBackend`].

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod backend;
mod bridge;
mod error;
mod ffi;

/// ABI version expected from `lattice_capture_bridge_abi_version()`.
pub const LATTICE_CAPTURE_BRIDGE_ABI_VERSION: u32 = 1;

pub use backend::MacOsCaptureBackend;
pub use error::ensure_abi_version;

/// Returns the ABI version this crate expects from the native bridge.
#[must_use]
pub fn expected_bridge_abi_version() -> u32 {
    LATTICE_CAPTURE_BRIDGE_ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(LATTICE_CAPTURE_BRIDGE_ABI_VERSION, 1);
        assert_eq!(expected_bridge_abi_version(), 1);
    }

    #[test]
    fn swift_package_path_exists() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(manifest_dir.join("swift/Package.swift").is_file());
        assert!(manifest_dir.join("include/lattice_capture_bridge.h").is_file());
    }

    #[test]
    fn bridge_lib_env_path_validation() {
        let invalid = std::env::var("LATTICE_CAPTURE_BRIDGE_LIB").unwrap_or_default();
        if !invalid.is_empty() {
            let path = Path::new(&invalid);
            assert!(
                path.is_dir(),
                "LATTICE_CAPTURE_BRIDGE_LIB must be a directory when set"
            );
        }
    }
}
