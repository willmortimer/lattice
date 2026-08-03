//! Windows capture adapter for Lattice clipper.
//!
//! This crate is a compile-time stub that implements
//! [`lattice_capture_core::CaptureBackend`] with clear
//! [`CaptureError::Unsupported`](lattice_capture_core::CaptureError::Unsupported)
//! results. Real Windows Graphics Capture (WGC) pixel capture replaces these
//! bodies in a follow-up task.

mod backend;
mod permission;

pub use backend::WindowsCaptureBackend;
pub use permission::{platform_permission_provider, WindowsCapturePermissionProvider};
