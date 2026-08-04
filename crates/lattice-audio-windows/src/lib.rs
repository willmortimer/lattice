//! Windows WASAPI microphone capture for Lattice voice (via cpal).
//!
//! On Windows this crate implements [`lattice_audio::CaptureProvider`] using
//! the default input device. On non-Windows hosts the public types compile and
//! return [`CaptureError::Unsupported`](lattice_audio::CaptureError::Unsupported)
//! so unit tests stay green without WASAPI.

mod provider;

#[cfg(windows)]
mod stream;

pub use provider::{default_input_available, WindowsCaptureProvider};
