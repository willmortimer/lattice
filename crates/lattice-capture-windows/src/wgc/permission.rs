//! Best-effort WGC permission status + settings deep-link.

use lattice_capture_core::{CapturePermissionState, CapturePermissionStatus};
use windows::core::{w, PCWSTR};
use windows::Graphics::Capture::GraphicsCaptureSession;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::permission::WINDOWS_CAPTURE_REASON;

use super::device::CaptureDevice;

pub fn permission_status(_request: bool) -> CapturePermissionStatus {
    // Ensure COM is up before probing WinRT statics when possible.
    let _ = CaptureDevice::new();

    let supported = GraphicsCaptureSession::IsSupported().unwrap_or(false);
    if supported {
        CapturePermissionStatus {
            state: CapturePermissionState::Authorized,
            available: true,
            platform: "windows".into(),
            reason: WINDOWS_CAPTURE_REASON.into(),
            message: Some(
                "Win32 apps lack a reliable screen-capture privacy query; WGC IsSupported is treated as available."
                    .into(),
            ),
        }
    } else {
        CapturePermissionStatus {
            state: CapturePermissionState::Unsupported,
            available: false,
            platform: "windows".into(),
            reason: WINDOWS_CAPTURE_REASON.into(),
            message: Some(
                "Windows Graphics Capture is not supported on this OS/build.".into(),
            ),
        }
    }
}

pub fn open_capture_settings() -> Result<(), String> {
    // Windows 11 graphics-capture privacy page when present; Settings hosts
    // older builds to the closest privacy surface.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("ms-settings:privacy-graphicscapture"),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize <= 32 {
        return Err(format!(
            "failed to open capture settings (ShellExecuteW={})",
            result.0 as usize
        ));
    }
    Ok(())
}
