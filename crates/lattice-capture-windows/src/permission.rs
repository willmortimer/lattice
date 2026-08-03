//! Windows capture permission provider (best-effort WGC availability).

use lattice_capture_core::{CapturePermissionProvider, CapturePermissionStatus};
#[cfg(not(windows))]
use lattice_capture_core::CapturePermissionState;

/// Why Lattice needs screen capture on Windows.
pub const WINDOWS_CAPTURE_REASON: &str = "Lattice uses Windows Graphics Capture to take screenshots and clips on this PC. Images stay on this device and are saved to your workspace Capture Inbox.";

/// Windows permission provider.
///
/// Win32 desktop apps do not have a macOS-style screen-recording TCC gate that
/// can be queried reliably. This provider reports:
/// - [`CapturePermissionState::Authorized`] when `GraphicsCaptureSession::IsSupported`
/// - [`CapturePermissionState::Unsupported`] when WGC is unavailable
/// - never blocks capture solely on an unreadable privacy toggle
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsCapturePermissionProvider;

impl CapturePermissionProvider for WindowsCapturePermissionProvider {
    fn status(&self) -> CapturePermissionStatus {
        #[cfg(windows)]
        {
            crate::wgc::permission_status(false)
        }
        #[cfg(not(windows))]
        {
            let _ = self;
            CapturePermissionStatus {
                state: CapturePermissionState::Unsupported,
                available: false,
                platform: std::env::consts::OS.into(),
                reason: WINDOWS_CAPTURE_REASON.into(),
                message: Some(
                    "Windows Graphics Capture permission APIs are only available on Windows builds"
                        .into(),
                ),
            }
        }
    }

    fn request(&self) -> CapturePermissionStatus {
        #[cfg(windows)]
        {
            // WGC for Win32 has no separate request prompt; re-query support.
            crate::wgc::permission_status(true)
        }
        #[cfg(not(windows))]
        {
            self.status()
        }
    }

    fn open_settings(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            crate::wgc::open_capture_settings()
        }
        #[cfg(not(windows))]
        {
            let _ = self;
            Err("screen capture settings are only available on Windows".into())
        }
    }
}

/// Returns the platform permission provider for the current build.
pub fn platform_permission_provider() -> WindowsCapturePermissionProvider {
    WindowsCapturePermissionProvider
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_returns_windows_reason_copy() {
        let status = WindowsCapturePermissionProvider.status();
        assert_eq!(status.reason, WINDOWS_CAPTURE_REASON);
        assert_eq!(status.platform, std::env::consts::OS);
    }

    #[test]
    #[cfg(not(windows))]
    fn provider_reports_unsupported_off_windows() {
        let status = WindowsCapturePermissionProvider.status();
        assert!(!status.available);
        assert_eq!(status.state, CapturePermissionState::Unsupported);
    }
}
