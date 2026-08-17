//! Bindgen-free `extern "C"` bindings for `include/lattice_capture_bridge.h`.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use std::os::raw::c_void;

pub const LATTICE_CAPTURE_OK: i32 = 0;
pub const LATTICE_CAPTURE_ERR_INVALID_ARG: i32 = -1;
pub const LATTICE_CAPTURE_ERR_CANCELLED: i32 = -2;
pub const LATTICE_CAPTURE_ERR_PERMISSION: i32 = -3;
pub const LATTICE_CAPTURE_ERR_NOT_FOUND: i32 = -4;
pub const LATTICE_CAPTURE_ERR_INTERNAL: i32 = -5;
pub const LATTICE_CAPTURE_ERR_UNSUPPORTED: i32 = -6;
pub const LATTICE_CAPTURE_ERR_NOT_IMPLEMENTED: i32 = -7;

pub const LATTICE_CAPTURE_PERM_UNSUPPORTED: u32 = 0;
pub const LATTICE_CAPTURE_PERM_NOT_DETERMINED: u32 = 1;
pub const LATTICE_CAPTURE_PERM_AUTHORIZED: u32 = 2;
pub const LATTICE_CAPTURE_PERM_DENIED: u32 = 3;
pub const LATTICE_CAPTURE_PERM_RESTRICTED: u32 = 4;

pub const LATTICE_CAPTURE_WINDOW_TITLE_MAX: usize = 256;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LatticeCapturePermissionStatus {
    pub state: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LatticeCaptureDisplayInfo {
    pub display_id: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct LatticeCaptureImageOut {
    pub width: u32,
    pub height: u32,
    pub png_bytes: *mut u8,
    pub png_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LatticeCaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LatticeCaptureWindowInfo {
    pub window_id: u64,
    pub width: u32,
    pub height: u32,
    pub title: [u8; LATTICE_CAPTURE_WINDOW_TITLE_MAX],
}

impl LatticeCaptureWindowInfo {
    /// UTF-8 title copied from the C ABI buffer (NUL-terminated).
    pub fn title_string(&self) -> Option<String> {
        title_from_c_bytes(&self.title)
    }
}

/// Decode a NUL-terminated UTF-8 title from a fixed C ABI buffer.
pub fn title_from_c_bytes(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    let slice = &bytes[..end];
    if slice.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(slice).into_owned())
}

#[cfg(link_bridge)]
#[link(name = "LatticeCaptureBridge", kind = "dylib")]
extern "C" {
    pub fn lattice_capture_bridge_abi_version() -> u32;

    pub fn lattice_capture_enumerate_displays(
        out: *mut LatticeCaptureDisplayInfo,
        out_capacity: u32,
        out_count: *mut u32,
    ) -> i32;

    pub fn lattice_capture_enumerate_windows(
        out: *mut LatticeCaptureWindowInfo,
        out_capacity: u32,
        out_count: *mut u32,
    ) -> i32;

    pub fn lattice_capture_capture_display(
        display_id: u32,
        out_image: *mut LatticeCaptureImageOut,
    ) -> i32;

    pub fn lattice_capture_capture_window(
        window_id: u64,
        out_image: *mut LatticeCaptureImageOut,
    ) -> i32;

    pub fn lattice_capture_capture_region(
        display_id: u32,
        region: *const LatticeCaptureRegion,
        out_image: *mut LatticeCaptureImageOut,
    ) -> i32;

    pub fn lattice_capture_select_interactive_region(
        out_display_id: *mut u32,
        out_region: *mut LatticeCaptureRegion,
    ) -> i32;

    pub fn lattice_capture_select_interactive_window(out_window_id: *mut u64) -> i32;

    pub fn lattice_capture_capture_interactive_region(
        out_image: *mut LatticeCaptureImageOut,
    ) -> i32;

    pub fn lattice_capture_image_release(image: *mut LatticeCaptureImageOut);

    pub fn lattice_capture_permission_status(
        out_status: *mut LatticeCapturePermissionStatus,
    ) -> i32;

    pub fn lattice_capture_permission_request(
        out_status: *mut LatticeCapturePermissionStatus,
    ) -> i32;

    pub fn lattice_capture_permission_open_settings() -> i32;
}

/// Copy PNG bytes from a bridge-owned image and release native memory.
#[cfg(link_bridge)]
pub(crate) fn take_png_image(mut image: LatticeCaptureImageOut) -> BridgeImage {
    let png = if image.png_bytes.is_null() || image.png_len == 0 {
        Vec::new()
    } else {
        let len = image.png_len as usize;
        let bytes =
            unsafe { std::slice::from_raw_parts(image.png_bytes as *const u8, len) }.to_vec();
        unsafe {
            lattice_capture_image_release(&mut image);
        }
        bytes
    };
    BridgeImage {
        width: image.width,
        height: image.height,
        png_bytes: png,
    }
}

#[cfg(not(link_bridge))]
pub(crate) fn take_png_image(_image: LatticeCaptureImageOut) -> BridgeImage {
    unreachable!("take_png_image requires link_bridge")
}

pub(crate) struct BridgeImage {
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
}

// Silence unused import on non-macOS targets.
#[allow(dead_code)]
type _Unused = c_void;
