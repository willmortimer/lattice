//! Capture request plans.

use crate::{CaptureDestination, CaptureSource};

/// Static screenshot request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotPlan {
    pub source: CaptureSource,
    pub destination: CaptureDestination,
}

/// Screen recording request (stub; backends default to unsupported).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePlan {
    pub source: CaptureSource,
    pub destination: CaptureDestination,
}
