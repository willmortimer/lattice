//! Stub [`CaptureBackend`] for Windows until WGC lands.

use lattice_capture_core::{
    CaptureBackend, CaptureError, CaptureSourceInfo, CapturedImage, ScreenshotPlan,
};

const UNSUPPORTED_MSG: &str =
    "Windows Graphics Capture is not implemented yet (lattice-capture-windows stub)";

/// Windows capture backend placeholder.
///
/// All capture operations return [`CaptureError::Unsupported`] until WGC is wired.
#[derive(Debug, Default)]
pub struct WindowsCaptureBackend;

impl WindowsCaptureBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CaptureBackend for WindowsCaptureBackend {
    fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>, CaptureError> {
        Err(CaptureError::Unsupported(UNSUPPORTED_MSG.into()))
    }

    fn screenshot(&self, _plan: ScreenshotPlan) -> Result<CapturedImage, CaptureError> {
        Err(CaptureError::Unsupported(UNSUPPORTED_MSG.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_capture_core::{
        CaptureDestination, CapturePlan, CaptureSource, DisplayHandle,
    };

    #[test]
    fn enumerate_is_unsupported() {
        let backend = WindowsCaptureBackend::new();
        let err = backend.enumerate_sources().unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
        assert!(err.to_string().contains("Windows Graphics Capture"));
    }

    #[test]
    fn screenshot_is_unsupported() {
        let backend = WindowsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::Display(DisplayHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    fn interactive_region_is_unsupported() {
        let backend = WindowsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::InteractiveRegion,
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    fn recording_default_is_unsupported() {
        let backend = WindowsCaptureBackend::new();
        let plan = CapturePlan {
            source: CaptureSource::Display(DisplayHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = match backend.begin_recording(plan) {
            Err(err) => err,
            Ok(_) => panic!("expected unsupported recording"),
        };
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }
}
