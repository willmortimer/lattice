//! [`CaptureBackend`] for Windows Graphics Capture (WGC).

use lattice_capture_core::{
    CaptureBackend, CaptureError, CaptureSourceInfo, CapturedImage, ScreenshotPlan,
};
#[cfg(windows)]
use lattice_capture_core::CaptureSource;

#[cfg(not(windows))]
const UNSUPPORTED_HOST_MSG: &str =
    "Windows Graphics Capture requires a Windows host (lattice-capture-windows)";

/// Windows capture backend backed by WGC on `cfg(windows)`.
#[derive(Debug, Default)]
pub struct WindowsCaptureBackend;

impl WindowsCaptureBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CaptureBackend for WindowsCaptureBackend {
    fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>, CaptureError> {
        #[cfg(windows)]
        {
            crate::wgc::enumerate_display_sources()
        }
        #[cfg(not(windows))]
        {
            Err(CaptureError::Unsupported(UNSUPPORTED_HOST_MSG.into()))
        }
    }

    fn screenshot(&self, plan: ScreenshotPlan) -> Result<CapturedImage, CaptureError> {
        #[cfg(windows)]
        {
            match plan.source {
                CaptureSource::Display(display) => crate::wgc::capture_display(display),
                CaptureSource::Region(region) => crate::wgc::capture_region(region),
                CaptureSource::InteractiveRegion => crate::wgc::capture_interactive_region(),
                CaptureSource::Window(_) => Err(CaptureError::Unsupported(
                    "window capture is not implemented yet on Windows".into(),
                )),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = plan;
            Err(CaptureError::Unsupported(UNSUPPORTED_HOST_MSG.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_capture_core::{CaptureDestination, CapturePlan, CaptureSource, DisplayHandle};

    #[test]
    #[cfg(not(windows))]
    fn enumerate_is_unsupported_off_windows() {
        let backend = WindowsCaptureBackend::new();
        let err = backend.enumerate_sources().unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
        assert!(err.to_string().contains("Windows Graphics Capture"));
    }

    #[test]
    #[cfg(not(windows))]
    fn screenshot_is_unsupported_off_windows() {
        let backend = WindowsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::Display(DisplayHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    #[cfg(not(windows))]
    fn interactive_region_is_unsupported_off_windows() {
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

    #[test]
    #[cfg(windows)]
    fn window_source_is_unsupported() {
        let backend = WindowsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::Window(lattice_capture_core::WindowHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
        assert!(err.to_string().contains("window capture"));
    }

    #[test]
    #[cfg(windows)]
    fn region_handle_shape_is_accepted_by_plan() {
        // Compile/shape guard: region plans stay first-class even when GPU
        // capture is skipped in unit tests.
        let plan = ScreenshotPlan {
            source: CaptureSource::Region(lattice_capture_core::RegionHandle {
                display_id: 1,
                x: 10,
                y: 20,
                width: 100,
                height: 80,
            }),
            destination: CaptureDestination::CaptureInbox,
        };
        assert!(matches!(plan.source, CaptureSource::Region(_)));
    }
}
