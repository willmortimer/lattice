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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaptureSource, DisplayHandle, RegionHandle, WindowHandle};

    #[test]
    fn screenshot_plan_carries_source_and_destination() {
        let plan = ScreenshotPlan {
            source: CaptureSource::InteractiveRegion,
            destination: CaptureDestination::Clipboard,
        };
        assert_eq!(plan.source, CaptureSource::InteractiveRegion);
        assert_eq!(plan.destination, CaptureDestination::Clipboard);
    }

    #[test]
    fn capture_plan_supports_display_and_inbox_routing() {
        let plan = CapturePlan {
            source: CaptureSource::Display(DisplayHandle(3)),
            destination: CaptureDestination::CaptureInbox,
        };
        assert_eq!(plan.source, CaptureSource::Display(DisplayHandle(3)));
        assert_eq!(plan.destination, CaptureDestination::CaptureInbox);
    }

    #[test]
    fn plans_differ_when_region_geometry_differs() {
        let region_a = CaptureSource::Region(RegionHandle {
            display_id: 1,
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        });
        let region_b = CaptureSource::Region(RegionHandle {
            display_id: 1,
            x: 10,
            y: 20,
            width: 50,
            height: 50,
        });
        let plan_a = ScreenshotPlan {
            source: region_a,
            destination: CaptureDestination::CurrentNote,
        };
        let plan_b = ScreenshotPlan {
            source: region_b,
            destination: CaptureDestination::CurrentNote,
        };
        assert_ne!(plan_a, plan_b);
    }

    #[test]
    fn window_source_round_trips_in_plan() {
        let plan = CapturePlan {
            source: CaptureSource::Window(WindowHandle(99)),
            destination: CaptureDestination::NamedCollection("clips".into()),
        };
        assert_eq!(plan.source, CaptureSource::Window(WindowHandle(99)));
        assert_eq!(
            plan.destination,
            CaptureDestination::NamedCollection("clips".into())
        );
    }
}
