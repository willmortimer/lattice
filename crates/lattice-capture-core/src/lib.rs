//! Provider-neutral screen capture types for Lattice clipper.
//!
//! Platform capture (ScreenCaptureKit, AppKit) lives in `lattice-capture-macos`.

mod backend;
mod destination;
mod error;
mod image;
mod permission;
mod plan;
mod recording;
mod source;

pub use backend::CaptureBackend;
pub use destination::CaptureDestination;
pub use error::CaptureError;
pub use image::{CapturedImage, ImageData};
pub use permission::{
    CapturePermissionProvider, CapturePermissionState, CapturePermissionStatus,
    SCREEN_RECORDING_REASON, UnsupportedCapturePermissionProvider,
};
pub use plan::{CapturePlan, ScreenshotPlan};
pub use recording::RecordingSession;
pub use source::{
    CaptureSource, CaptureSourceInfo, DisplayHandle, RegionHandle, WindowHandle,
};
