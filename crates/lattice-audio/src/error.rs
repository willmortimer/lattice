//! Capture error surface.

use thiserror::Error;

/// Errors from a [`crate::CaptureProvider`] or local buffer operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CaptureError {
    #[error("capture is not armed")]
    NotArmed,
    #[error("capture is already running")]
    AlreadyRunning,
    #[error("capture is not running")]
    NotRunning,
    #[error("microphone permission denied")]
    PermissionDenied,
    #[error("audio device error: {0}")]
    Device(String),
    #[error("event subscriber disconnected")]
    EventSubscriberDisconnected,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("provider error: {0}")]
    Provider(String),
}

impl CaptureError {
    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider(message.into())
    }

    pub fn device(message: impl Into<String>) -> Self {
        Self::Device(message.into())
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }
}
