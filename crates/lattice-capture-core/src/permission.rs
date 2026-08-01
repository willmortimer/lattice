//! Screen recording permission types and platform-neutral surface.

use serde::{Deserialize, Serialize};

/// macOS Screen Recording / ScreenCaptureKit permission state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CapturePermissionState {
    /// Platform or build does not expose capture permission APIs.
    Unsupported,
    /// User has not been prompted yet.
    NotDetermined,
    /// Screen recording is allowed for this app.
    Authorized,
    /// User denied screen recording for this app.
    Denied,
    /// Screen recording is blocked by policy (managed device).
    Restricted,
}

impl CapturePermissionState {
    pub fn is_authorized(self) -> bool {
        matches!(self, Self::Authorized)
    }
}

/// Typed permission snapshot returned to desktop/daemon callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePermissionStatus {
    pub state: CapturePermissionState,
    /// Whether the current build can query/request screen recording permission.
    pub available: bool,
    pub platform: String,
    /// Why Lattice needs screen recording access.
    pub reason: String,
    /// Optional diagnostic detail for settings UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl CapturePermissionStatus {
    pub fn unsupported(platform: impl Into<String>) -> Self {
        Self {
            state: CapturePermissionState::Unsupported,
            available: false,
            platform: platform.into(),
            reason: SCREEN_RECORDING_REASON.into(),
            message: None,
        }
    }
}

/// User-facing explanation for why screen recording permission is required.
pub const SCREEN_RECORDING_REASON: &str = "Lattice uses screen recording to capture screenshots and clips on your Mac. Images stay on this device and are saved to your workspace Capture Inbox.";

/// Platform-neutral permission provider.
pub trait CapturePermissionProvider {
    fn status(&self) -> CapturePermissionStatus;

    /// Prompt the user when permission is not yet determined.
    fn request(&self) -> CapturePermissionStatus;

    /// Open the OS privacy settings pane for screen recording.
    fn open_settings(&self) -> Result<(), String>;
}

/// Stub provider for non-macOS targets and unit tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedCapturePermissionProvider;

impl CapturePermissionProvider for UnsupportedCapturePermissionProvider {
    fn status(&self) -> CapturePermissionStatus {
        CapturePermissionStatus::unsupported(std::env::consts::OS)
    }

    fn request(&self) -> CapturePermissionStatus {
        self.status()
    }

    fn open_settings(&self) -> Result<(), String> {
        Err("screen recording settings are not available on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_state_is_detected() {
        assert!(CapturePermissionState::Authorized.is_authorized());
        assert!(!CapturePermissionState::Denied.is_authorized());
    }

    #[test]
    fn reason_is_non_empty() {
        assert!(!SCREEN_RECORDING_REASON.is_empty());
    }

    #[test]
    fn unsupported_provider_reports_unavailable() {
        let provider = UnsupportedCapturePermissionProvider;
        let status = provider.status();
        assert!(!status.available);
        assert_eq!(status.state, CapturePermissionState::Unsupported);
    }
}
