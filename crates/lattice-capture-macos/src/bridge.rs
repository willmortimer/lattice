//! Thin wrappers around the linked LatticeCaptureBridge C ABI.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use crate::error::BridgeResult;
#[cfg(link_bridge)]
use crate::error::{ensure_abi_version, map_status};
#[cfg(link_bridge)]
use crate::ffi;
#[cfg(link_bridge)]
use crate::ffi::LatticeCaptureImageOut;
use crate::ffi::{
    BridgeImage, LatticeCaptureDisplayInfo, LatticeCaptureRegion, LatticeCaptureWindowInfo,
};
#[cfg(link_bridge)]
use crate::LATTICE_CAPTURE_BRIDGE_ABI_VERSION;
#[cfg(not(link_bridge))]
use lattice_capture_core::CaptureError;

pub struct NativeBridge;

impl NativeBridge {
    #[cfg(link_bridge)]
    pub fn ensure_linked() -> BridgeResult<()> {
        unsafe {
            let actual = ffi::lattice_capture_bridge_abi_version();
            ensure_abi_version(LATTICE_CAPTURE_BRIDGE_ABI_VERSION, actual)
        }
    }

    #[cfg(not(link_bridge))]
    pub fn ensure_linked() -> BridgeResult<()> {
        Err(CaptureError::Unsupported(
            "LatticeCaptureBridge is not linked; build with --features link-bridge".into(),
        ))
    }

    #[cfg(link_bridge)]
    pub fn enumerate_displays() -> BridgeResult<Vec<LatticeCaptureDisplayInfo>> {
        Self::ensure_linked()?;
        const CAPACITY: u32 = 32;
        let mut rows = vec![
            LatticeCaptureDisplayInfo {
                display_id: 0,
                width: 0,
                height: 0,
            };
            CAPACITY as usize
        ];
        let mut count = 0u32;
        unsafe {
            map_status(
                ffi::lattice_capture_enumerate_displays(rows.as_mut_ptr(), CAPACITY, &mut count),
                "enumerate_displays",
            )?;
        }
        rows.truncate(count as usize);
        Ok(rows)
    }

    #[cfg(not(link_bridge))]
    pub fn enumerate_displays() -> BridgeResult<Vec<LatticeCaptureDisplayInfo>> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    #[cfg(link_bridge)]
    pub fn enumerate_windows() -> BridgeResult<Vec<LatticeCaptureWindowInfo>> {
        Self::ensure_linked()?;
        const CAPACITY: u32 = 256;
        let mut rows = vec![
            LatticeCaptureWindowInfo {
                window_id: 0,
                width: 0,
                height: 0,
                title: [0u8; crate::ffi::LATTICE_CAPTURE_WINDOW_TITLE_MAX],
            };
            CAPACITY as usize
        ];
        let mut count = 0u32;
        unsafe {
            map_status(
                ffi::lattice_capture_enumerate_windows(rows.as_mut_ptr(), CAPACITY, &mut count),
                "enumerate_windows",
            )?;
        }
        rows.truncate(count as usize);
        Ok(rows)
    }

    #[cfg(not(link_bridge))]
    pub fn enumerate_windows() -> BridgeResult<Vec<LatticeCaptureWindowInfo>> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    #[cfg(link_bridge)]
    pub fn capture_display(display_id: u32) -> BridgeResult<BridgeImage> {
        Self::ensure_linked()?;
        let mut out = LatticeCaptureImageOut {
            width: 0,
            height: 0,
            png_bytes: std::ptr::null_mut(),
            png_len: 0,
        };
        unsafe {
            map_status(
                ffi::lattice_capture_capture_display(display_id, &mut out),
                "capture_display",
            )?;
            Ok(ffi::take_png_image(out))
        }
    }

    #[cfg(not(link_bridge))]
    pub fn capture_display(_display_id: u32) -> BridgeResult<BridgeImage> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    #[cfg(link_bridge)]
    pub fn capture_window(window_id: u64) -> BridgeResult<BridgeImage> {
        Self::ensure_linked()?;
        let mut out = LatticeCaptureImageOut {
            width: 0,
            height: 0,
            png_bytes: std::ptr::null_mut(),
            png_len: 0,
        };
        unsafe {
            map_status(
                ffi::lattice_capture_capture_window(window_id, &mut out),
                "capture_window",
            )?;
            Ok(ffi::take_png_image(out))
        }
    }

    #[cfg(not(link_bridge))]
    pub fn capture_window(_window_id: u64) -> BridgeResult<BridgeImage> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    #[cfg(link_bridge)]
    pub fn capture_region(
        display_id: u32,
        region: LatticeCaptureRegion,
    ) -> BridgeResult<BridgeImage> {
        Self::ensure_linked()?;
        let mut out = LatticeCaptureImageOut {
            width: 0,
            height: 0,
            png_bytes: std::ptr::null_mut(),
            png_len: 0,
        };
        unsafe {
            map_status(
                ffi::lattice_capture_capture_region(display_id, &region, &mut out),
                "capture_region",
            )?;
            Ok(ffi::take_png_image(out))
        }
    }

    #[cfg(not(link_bridge))]
    pub fn capture_region(
        _display_id: u32,
        _region: LatticeCaptureRegion,
    ) -> BridgeResult<BridgeImage> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    /// Present the AppKit overlay and return display-local region geometry.
    ///
    /// Encode/ingest stay in Rust; this only performs interactive selection.
    #[cfg(link_bridge)]
    pub fn select_interactive_region() -> BridgeResult<(u32, LatticeCaptureRegion)> {
        Self::ensure_linked()?;
        let mut display_id = 0u32;
        let mut region = LatticeCaptureRegion {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        unsafe {
            map_status(
                ffi::lattice_capture_select_interactive_region(&mut display_id, &mut region),
                "select_interactive_region",
            )?;
        }
        Ok((display_id, region))
    }

    #[cfg(not(link_bridge))]
    pub fn select_interactive_region() -> BridgeResult<(u32, LatticeCaptureRegion)> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    /// Present the AppKit overlay and return the clicked window id.
    ///
    /// Encode/ingest stay in Rust; this only performs interactive selection.
    #[cfg(link_bridge)]
    pub fn select_interactive_window() -> BridgeResult<u64> {
        Self::ensure_linked()?;
        let mut window_id = 0u64;
        unsafe {
            map_status(
                ffi::lattice_capture_select_interactive_window(&mut window_id),
                "select_interactive_window",
            )?;
        }
        Ok(window_id)
    }

    #[cfg(not(link_bridge))]
    pub fn select_interactive_window() -> BridgeResult<u64> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    /// Select an interactive region, then capture via the fixed-region SCK path.
    #[cfg(link_bridge)]
    pub fn capture_interactive_region() -> BridgeResult<BridgeImage> {
        let (display_id, region) = Self::select_interactive_region()?;
        Self::capture_region(display_id, region)
    }

    #[cfg(not(link_bridge))]
    pub fn capture_interactive_region() -> BridgeResult<BridgeImage> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }
}
