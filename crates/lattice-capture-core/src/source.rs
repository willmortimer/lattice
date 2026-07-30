//! Capture source handles and enumeration metadata.

/// Opaque display identifier (platform-defined, typically `CGDirectDisplayID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayHandle(pub u32);

/// Opaque window identifier (platform-defined).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub u64);

/// Screen region in global display coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionHandle {
    pub display_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// What to capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSource {
    Display(DisplayHandle),
    Window(WindowHandle),
    Region(RegionHandle),
    /// Interactive region selection (overlay); backend may return
    /// [`crate::CaptureError::Cancelled`] when dismissed.
    InteractiveRegion,
}

/// One row from [`crate::CaptureBackend::enumerate_sources`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSourceInfo {
    pub source: CaptureSource,
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
