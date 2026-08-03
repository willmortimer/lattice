//! Windows capture permission stub (WGC permission wiring lands with real capture).

use lattice_capture_core::{
    CapturePermissionProvider, CapturePermissionStatus, UnsupportedCapturePermissionProvider,
};

/// Windows permission provider placeholder until WGC permission APIs are wired.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsCapturePermissionProvider;

impl CapturePermissionProvider for WindowsCapturePermissionProvider {
    fn status(&self) -> CapturePermissionStatus {
        UnsupportedCapturePermissionProvider.status()
    }

    fn request(&self) -> CapturePermissionStatus {
        UnsupportedCapturePermissionProvider.request()
    }

    fn open_settings(&self) -> Result<(), String> {
        UnsupportedCapturePermissionProvider.open_settings()
    }
}

/// Returns the platform permission provider for the current build.
pub fn platform_permission_provider() -> WindowsCapturePermissionProvider {
    WindowsCapturePermissionProvider
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_capture_core::CapturePermissionState;

    #[test]
    fn provider_reports_unsupported() {
        let status = WindowsCapturePermissionProvider.status();
        assert!(!status.available);
        assert_eq!(status.state, CapturePermissionState::Unsupported);
    }
}
