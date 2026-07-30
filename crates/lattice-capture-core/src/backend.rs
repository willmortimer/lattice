//! [`CaptureBackend`] trait and test helpers.

use crate::{
    CaptureError, CapturePlan, CaptureSourceInfo, CapturedImage, RecordingSession, ScreenshotPlan,
};

/// Platform-neutral screen capture surface.
pub trait CaptureBackend {
    fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>, CaptureError>;

    fn screenshot(&self, plan: ScreenshotPlan) -> Result<CapturedImage, CaptureError>;

    fn begin_recording(
        &self,
        _plan: CapturePlan,
    ) -> Result<Box<dyn RecordingSession>, CaptureError> {
        Err(CaptureError::Unsupported(
            "screen recording is not supported by this backend".into(),
        ))
    }
}

/// Minimal backend for unit tests (enumerate empty, screenshot unsupported).
#[cfg(test)]
pub(crate) struct TestStubBackend;

#[cfg(test)]
impl CaptureBackend for TestStubBackend {
    fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>, CaptureError> {
        Ok(Vec::new())
    }

    fn screenshot(&self, _plan: ScreenshotPlan) -> Result<CapturedImage, CaptureError> {
        Err(CaptureError::Unsupported("test stub".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CaptureDestination, CaptureSource, DisplayHandle, RegionHandle, WindowHandle,
    };

    #[test]
    fn capture_destination_variants() {
        let destinations = [
            CaptureDestination::Clipboard,
            CaptureDestination::CaptureInbox,
            CaptureDestination::CurrentNote,
            CaptureDestination::CurrentCanvas,
            CaptureDestination::NamedCollection("shots".into()),
        ];
        assert_eq!(destinations.len(), 5);
        assert_eq!(
            destinations[4],
            CaptureDestination::NamedCollection("shots".into())
        );
    }

    #[test]
    fn capture_source_variants() {
        let sources = [
            CaptureSource::Display(DisplayHandle(1)),
            CaptureSource::Window(WindowHandle(42)),
            CaptureSource::Region(RegionHandle {
                display_id: 1,
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }),
            CaptureSource::InteractiveRegion,
        ];
        assert_eq!(sources.len(), 4);
    }

    #[test]
    fn recording_stub_returns_unsupported() {
        let backend = TestStubBackend;
        let plan = CapturePlan {
            source: CaptureSource::Display(DisplayHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = match backend.begin_recording(plan) {
            Err(err) => err,
            Ok(_) => panic!("expected unsupported recording"),
        };
        assert!(matches!(err, CaptureError::Unsupported(_)));
        assert!(err.to_string().contains("recording"));
    }
}
