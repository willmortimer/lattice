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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_handle_preserves_geometry() {
        let region = RegionHandle {
            display_id: 2,
            x: 40,
            y: 80,
            width: 640,
            height: 480,
        };
        assert_eq!(region.display_id, 2);
        assert_eq!(region.width, 640);
        assert_eq!(region.height, 480);
    }

    #[test]
    fn capture_source_info_carries_optional_metadata() {
        let info = CaptureSourceInfo {
            source: CaptureSource::Window(WindowHandle(7)),
            title: Some("Lattice".into()),
            width: Some(1920),
            height: Some(1080),
        };
        assert_eq!(info.title.as_deref(), Some("Lattice"));
        assert_eq!(info.width, Some(1920));
        assert_eq!(info.height, Some(1080));
    }

    #[test]
    fn display_and_window_handles_are_hashable_keys() {
        use std::collections::HashSet;

        let mut displays = HashSet::new();
        assert!(displays.insert(DisplayHandle(1)));
        assert!(!displays.insert(DisplayHandle(1)));
        assert!(displays.insert(DisplayHandle(2)));

        let mut windows = HashSet::new();
        assert!(windows.insert(WindowHandle(42)));
        assert!(!windows.insert(WindowHandle(42)));
    }
}
