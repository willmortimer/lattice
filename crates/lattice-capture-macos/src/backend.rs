//! [`CaptureBackend`] backed by LatticeCaptureBridge (when linked).

use lattice_capture_core::{
    CaptureBackend, CaptureError, CaptureSource, CaptureSourceInfo, CapturedImage, DisplayHandle,
    ScreenshotPlan, WindowHandle,
};

use crate::bridge::NativeBridge;
use crate::ffi::{LatticeCaptureDisplayInfo, LatticeCaptureRegion, LatticeCaptureWindowInfo};

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

    /// Present an AppKit overlay; click a window to target it.
    ///
    /// Encode/ingest stay in Rust. Pair with [`CaptureSource::Window`].
    pub fn select_interactive_window(&self) -> Result<WindowHandle, CaptureError> {
        let window_id = NativeBridge::select_interactive_window()?;
        Ok(WindowHandle(window_id))
    }
}

impl CaptureBackend for MacOsCaptureBackend {
    fn enumerate_sources(&self) -> Result<Vec<CaptureSourceInfo>, CaptureError> {
        let displays = NativeBridge::enumerate_displays()?;
        let windows = NativeBridge::enumerate_windows()?;
        Ok(map_enumerated_sources(&displays, &windows))
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
            CaptureSource::Window(WindowHandle(id)) => NativeBridge::capture_window(id)?,
        };

        if image.png_bytes.is_empty() {
            return Err(CaptureError::internal("capture returned empty PNG"));
        }

        Ok(CapturedImage::png(
            image.width,
            image.height,
            image.png_bytes,
        ))
    }
}

fn map_enumerated_sources(
    displays: &[LatticeCaptureDisplayInfo],
    windows: &[LatticeCaptureWindowInfo],
) -> Vec<CaptureSourceInfo> {
    let mut sources: Vec<CaptureSourceInfo> = displays
        .iter()
        .map(|display| CaptureSourceInfo {
            source: CaptureSource::Display(DisplayHandle(display.display_id)),
            title: Some(format!("Display {}", display.display_id)),
            width: Some(display.width),
            height: Some(display.height),
        })
        .collect();
    sources.extend(windows.iter().map(source_info_from_window));
    sources
}

fn source_info_from_window(window: &LatticeCaptureWindowInfo) -> CaptureSourceInfo {
    CaptureSourceInfo {
        source: CaptureSource::Window(WindowHandle(window.window_id)),
        title: window.title_string(),
        width: Some(window.width),
        height: Some(window.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::{title_from_c_bytes, LATTICE_CAPTURE_WINDOW_TITLE_MAX};
    use lattice_capture_core::{CaptureDestination, CapturePlan, CaptureSource, DisplayHandle};

    fn window_info(id: u64, width: u32, height: u32, title: &str) -> LatticeCaptureWindowInfo {
        let mut bytes = [0u8; LATTICE_CAPTURE_WINDOW_TITLE_MAX];
        let encoded = title.as_bytes();
        let copy_len = encoded
            .len()
            .min(LATTICE_CAPTURE_WINDOW_TITLE_MAX.saturating_sub(1));
        bytes[..copy_len].copy_from_slice(&encoded[..copy_len]);
        LatticeCaptureWindowInfo {
            window_id: id,
            width,
            height,
            title: bytes,
        }
    }

    #[test]
    fn window_info_maps_to_window_source_with_title() {
        let info = source_info_from_window(&window_info(7, 1280, 720, "Notes"));
        assert_eq!(info.source, CaptureSource::Window(WindowHandle(7)));
        assert_eq!(info.title.as_deref(), Some("Notes"));
        assert_eq!(info.width, Some(1280));
        assert_eq!(info.height, Some(720));
    }

    #[test]
    fn window_title_stops_at_nul() {
        let mut info = window_info(1, 100, 100, "Safari");
        info.title[7] = b'X';
        assert_eq!(title_from_c_bytes(&info.title).as_deref(), Some("Safari"));
    }

    #[test]
    fn empty_window_title_maps_to_none() {
        let info = source_info_from_window(&window_info(3, 640, 480, ""));
        assert_eq!(info.title, None);
        assert_eq!(info.source, CaptureSource::Window(WindowHandle(3)));
    }

    #[test]
    fn enumerate_sources_maps_displays_then_windows() {
        let displays = [LatticeCaptureDisplayInfo {
            display_id: 1,
            width: 1920,
            height: 1080,
        }];
        let windows = [window_info(42, 800, 600, "Terminal")];
        let sources = map_enumerated_sources(&displays, &windows);
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].source, CaptureSource::Display(DisplayHandle(1)));
        assert_eq!(sources[1].source, CaptureSource::Window(WindowHandle(42)));
        assert_eq!(sources[1].title.as_deref(), Some("Terminal"));
    }

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
    #[cfg(not(link_bridge))]
    fn interactive_region_without_bridge_is_unsupported() {
        let backend = MacOsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::InteractiveRegion,
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
    }

    #[test]
    #[cfg(not(link_bridge))]
    fn window_screenshot_without_bridge_is_unsupported_not_unimplemented() {
        let backend = MacOsCaptureBackend::new();
        let plan = ScreenshotPlan {
            source: CaptureSource::Window(WindowHandle(1)),
            destination: CaptureDestination::CaptureInbox,
        };
        let err = backend.screenshot(plan).unwrap_err();
        assert!(matches!(err, CaptureError::Unsupported(_)));
        assert!(
            !err.to_string().contains("not implemented yet"),
            "window capture must call the bridge, not a hardcoded unimplemented stub: {err}"
        );
    }

    #[test]
    #[cfg(not(link_bridge))]
    fn select_interactive_window_without_bridge_is_unsupported() {
        let backend = MacOsCaptureBackend::new();
        let err = backend.select_interactive_window().unwrap_err();
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
