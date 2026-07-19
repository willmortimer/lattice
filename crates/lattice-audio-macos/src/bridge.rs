//! Thin wrappers around the linked LatticeAudioBridge C ABI.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use lattice_audio::CaptureError;

#[cfg(link_bridge)]
use crate::error::{ensure_abi_version, map_status};
use crate::error::BridgeResult;
use crate::ffi::{LatticeAudioCapture, LatticeAudioEventCallback};
#[cfg(link_bridge)]
use crate::ffi;
#[cfg(link_bridge)]
use crate::LATTICE_AUDIO_BRIDGE_ABI_VERSION;

/// Opaque capture handle backed by the Swift bridge (when linked).
pub struct NativeCapture {
    handle: LatticeAudioCapture,
}

impl NativeCapture {
    #[cfg(link_bridge)]
    pub fn create(pre_roll_ms: u32, enable_diagnostics: bool) -> BridgeResult<Self> {
        unsafe {
            let actual = ffi::lattice_audio_bridge_abi_version();
            ensure_abi_version(LATTICE_AUDIO_BRIDGE_ABI_VERSION, actual)?;

            let mut handle: LatticeAudioCapture = 0;
            let code = ffi::lattice_audio_capture_create(
                pre_roll_ms,
                u8::from(enable_diagnostics),
                &mut handle,
            );
            map_status(code, "lattice_audio_capture_create")?;
            if handle == 0 {
                return Err(CaptureError::provider(
                    "lattice_audio_capture_create returned a null handle",
                ));
            }
            Ok(Self { handle })
        }
    }

    #[cfg(not(link_bridge))]
    pub fn create(_pre_roll_ms: u32, _enable_diagnostics: bool) -> BridgeResult<Self> {
        Err(CaptureError::Unsupported(
            "LatticeAudioBridge is not linked; build with --features link-bridge".into(),
        ))
    }

    #[cfg(link_bridge)]
    pub fn arm(&self) -> BridgeResult<()> {
        unsafe { map_status(ffi::lattice_audio_capture_arm(self.handle), "arm") }
    }

    #[cfg(not(link_bridge))]
    pub fn arm(&self) -> BridgeResult<()> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    #[cfg(link_bridge)]
    pub fn start(
        &self,
        callback: LatticeAudioEventCallback,
        context: *mut std::ffi::c_void,
    ) -> BridgeResult<()> {
        unsafe {
            map_status(
                ffi::lattice_audio_capture_start(self.handle, callback, context),
                "start",
            )
        }
    }

    #[cfg(not(link_bridge))]
    pub fn start(
        &self,
        _callback: LatticeAudioEventCallback,
        _context: *mut std::ffi::c_void,
    ) -> BridgeResult<()> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }

    #[cfg(link_bridge)]
    pub fn stop(&self) -> BridgeResult<()> {
        unsafe { map_status(ffi::lattice_audio_capture_stop(self.handle), "stop") }
    }

    #[cfg(not(link_bridge))]
    pub fn stop(&self) -> BridgeResult<()> {
        Err(CaptureError::Unsupported("bridge not linked".into()))
    }
}

impl Drop for NativeCapture {
    fn drop(&mut self) {
        #[cfg(link_bridge)]
        unsafe {
            ffi::lattice_audio_capture_destroy(self.handle);
        }
    }
}
