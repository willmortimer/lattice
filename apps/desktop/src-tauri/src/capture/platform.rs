//! Platform capture backend + permission provider selection.

#[cfg(target_os = "macos")]
pub use lattice_capture_macos::{
    platform_permission_provider, MacOsCaptureBackend as PlatformCaptureBackend,
};

#[cfg(target_os = "windows")]
pub use lattice_capture_windows::{
    platform_permission_provider, WindowsCaptureBackend as PlatformCaptureBackend,
};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
compile_error!("feature `capture` requires macOS or Windows");
