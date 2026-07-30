//! [`CaptureBackend`] backed by LatticeCaptureBridge (when linked).

use lattice_capture_core::{
    CaptureBackend, CaptureError, CaptureSource, CaptureSourceInfo, CapturedImage, DisplayHandle,
    ScreenshotPlan,
};

use crate::bridge::NativeBridge;
use crate::ffi::LatticeCaptureRegion;

/// macOS ScreenCaptureKit capture backend.
///
/// Without the `link-bridge` feature this type still compiles but all capture
/// calls return [`CaptureError::Unsupported`].
#[derive(Debug, Default)]
pub struct MacOsCaptureBackend;

impl MacOsCaptureBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CaptureBackend for MacOsCaptureBackend {
    fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>, CaptureError> {
        let displays = NativeBridge::enumerate_displays()?;
        Ok(displays
            .into_iter()
            .map(|display| CaptureSourceInfo {
                source: CaptureSource::Display(DisplayHandle(display.display_id)),
                title: Some(format!("Display {}", display.display_id)),
                width: Some(display.width),
                height: Some(display.height),
            })
            .collect())
    }

    fn screenshot(&self, plan: ScreenshotPlan) -> Result<CapturedImage, CaptureError> {
        let image = match plan.source {
            CaptureSource::Display(DisplayHandle(id)) => NativeBridge::capture_display(id)?,
            CaptureSource::Region(region) => NativeBridge::capture_region(
                region.display_id,
                LatticeCaptureRegion {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: region.height,
                },
            )?,
            CaptureSource::InteractiveRegion => NativeBridge::capture_interactive_region()?,
            CaptureSource::Window(_) => {
                return Err(CaptureError::Unsupported(
                    "window capture is not implemented yet".into(),
                ));
            }
        };

        if image.png_bytes.is_empty() {
            return Err(CaptureError::internal("capture returned empty PNG"));
        }

        Ok(CapturedImage::png(image.width, image.height, image.png_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_capture_core::{CaptureDestination, CapturePlan, CaptureSource, DisplayHandle};

    #[test]
    #[cfg(not(link_bridge))]
    fn enumerate_without_bridge_is_unsupported() {
        let backend = MacOsCaptureBackend::new();
        let err = backend.enumerate_sources().unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    #[cfg(not(link_bridge))]
    fn screenshot_without_bridge_is_unsupported() {
        let backend = MacOsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::Display(DisplayHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    fn recording_default_is_unsupported() {
        let backend = MacOsCaptureBackend::new();
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
