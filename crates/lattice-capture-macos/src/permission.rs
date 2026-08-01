//! macOS screen recording permission via LatticeCaptureBridge.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use lattice_capture_core::{
    CapturePermissionProvider, CapturePermissionState, CapturePermissionStatus,
    SCREEN_RECORDING_REASON,
};

#[cfg(all(target_os = "macos", link_bridge))]
use crate::bridge::NativeBridge;
#[cfg(all(target_os = "macos", link_bridge))]
use crate::error::map_status;
#[cfg(all(target_os = "macos", link_bridge))]
use crate::ffi::{
    LatticeCapturePermissionStatus, LATTICE_CAPTURE_PERM_AUTHORIZED,
    LATTICE_CAPTURE_PERM_DENIED, LATTICE_CAPTURE_PERM_NOT_DETERMINED,
    LATTICE_CAPTURE_PERM_RESTRICTED, LATTICE_CAPTURE_PERM_UNSUPPORTED,
};

/// macOS permission provider backed by the Swift bridge when linked.
#[derive(Debug, Clone, Copy, Default)]
pub struct MacOsCapturePermissionProvider;

impl CapturePermissionProvider for MacOsCapturePermissionProvider {
    fn status(&self) -> CapturePermissionStatus {
        #[cfg(all(target_os = "macos", link_bridge))]
        {
            let _ = self;
            read_status(false).unwrap_or_else(|message| unsupported_status(Some(message)))
        }
        #[cfg(not(all(target_os = "macos", link_bridge)))]
        {
            let _ = self;
            unsupported_status(None)
        }
    }

    fn request(&self) -> CapturePermissionStatus {
        #[cfg(all(target_os = "macos", link_bridge))]
        {
            let _ = self;
            read_status(true).unwrap_or_else(|message| unsupported_status(Some(message)))
        }
        #[cfg(not(all(target_os = "macos", link_bridge)))]
        {
            let _ = self;
            unsupported_status(None)
        }
    }

    fn open_settings(&self) -> Result<(), String> {
        #[cfg(all(target_os = "macos", link_bridge))]
        {
            let _ = self;
            unsafe {
                map_status(
                    crate::ffi::lattice_capture_permission_open_settings(),
                    "permission_open_settings",
                )
                .map_err(|err| err.to_string())
            }
        }
        #[cfg(not(all(target_os = "macos", link_bridge)))]
        {
            let _ = self;
            Err("screen recording settings are not available on this platform".into())
        }
    }
}

/// Returns the platform permission provider for the current build.
pub fn platform_permission_provider() -> MacOsCapturePermissionProvider {
    MacOsCapturePermissionProvider
}

#[cfg(all(target_os = "macos", link_bridge))]
fn read_status(request: bool) -> Result<CapturePermissionStatus, String> {
    NativeBridge::ensure_linked().map_err(|err| err.to_string())?;
    let mut out = LatticeCapturePermissionStatus { state: 0 };
    unsafe {
        let code = if request {
            crate::ffi::lattice_capture_permission_request(&mut out)
        } else {
            crate::ffi::lattice_capture_permission_status(&mut out)
        };
        map_status(code, "permission_status").map_err(|err| err.to_string())?;
    }
    Ok(map_native_status(out.state))
}

#[cfg(all(target_os = "macos", link_bridge))]
fn map_native_status(raw: u32) -> CapturePermissionStatus {
    let state = match raw {
        LATTICE_CAPTURE_PERM_AUTHORIZED => CapturePermissionState::Authorized,
        LATTICE_CAPTURE_PERM_NOT_DETERMINED => CapturePermissionState::NotDetermined,
        LATTICE_CAPTURE_PERM_DENIED => CapturePermissionState::Denied,
        LATTICE_CAPTURE_PERM_RESTRICTED => CapturePermissionState::Restricted,
        LATTICE_CAPTURE_PERM_UNSUPPORTED | _ => CapturePermissionState::Unsupported,
    };
    CapturePermissionStatus {
        available: state != CapturePermissionState::Unsupported,
        state,
        platform: "macos".into(),
        reason: SCREEN_RECORDING_REASON.into(),
        message: None,
    }
}

fn unsupported_status(message: Option<String>) -> CapturePermissionStatus {
    CapturePermissionStatus {
        state: CapturePermissionState::Unsupported,
        available: false,
        platform: std::env::consts::OS.into(),
        reason: SCREEN_RECORDING_REASON.into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_returns_reason_copy() {
        let status = MacOsCapturePermissionProvider.status();
        assert_eq!(status.reason, SCREEN_RECORDING_REASON);
    }
}
