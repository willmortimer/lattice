//! Capture error surface.

use thiserror::Error;

/// Errors from a [`crate::CaptureBackend`] or local capture operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CaptureError {
    #[error("capture cancelled by user")]
    Cancelled,
    #[error("screen recording permission denied")]
    PermissionDenied,
    #[error("capture source not found: {0}")]
    NotFound(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl CaptureError {
    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider(message.into())
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_constructors_set_expected_variants() {
        assert!(matches!(
            CaptureError::provider("sck"),
            CaptureError::Provider(message) if message == "sck"
        ));
        assert!(matches!(
            CaptureError::invalid_argument("bad region"),
            CaptureError::InvalidArgument(message) if message == "bad region"
        ));
        assert!(matches!(
            CaptureError::not_found("display 9"),
            CaptureError::NotFound(message) if message == "display 9"
        ));
    }

    #[test]
    fn cancelled_and_permission_denied_are_user_actionable() {
        assert_eq!(
            CaptureError::Cancelled.to_string(),
            "capture cancelled by user"
        );
        assert_eq!(
            CaptureError::PermissionDenied.to_string(),
            "screen recording permission denied"
        );
    }
}
