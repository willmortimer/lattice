//! Windows capture adapter for Lattice clipper.
//!
//! On Windows this crate implements [`lattice_capture_core::CaptureBackend`] via
//! **Windows Graphics Capture** (WGC) for still images (display + region). On
//! non-Windows hosts the public types compile and return
//! [`CaptureError::Unsupported`](lattice_capture_core::CaptureError::Unsupported)
//! so unit tests stay green without a GPU or Windows SDK.

mod backend;
mod permission;

#[cfg(windows)]
mod wgc;

pub use backend::WindowsCaptureBackend;
pub use permission::{platform_permission_provider, WindowsCapturePermissionProvider};
