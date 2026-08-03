//! Windows Graphics Capture still-image pipeline.

mod capture;
mod device;
mod display;
mod encode;
mod exclusion;
mod permission;
mod picker;

pub use capture::{capture_display, capture_interactive_region, capture_region};
pub use display::enumerate_display_sources;
pub use permission::{open_capture_settings, permission_status};
