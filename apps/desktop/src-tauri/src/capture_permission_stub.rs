//! Stub screen-capture permission commands when the `capture` feature is off.

use lattice_capture_core::CapturePermissionStatus;

#[tauri::command]
pub fn capture_permission_status() -> Result<CapturePermissionStatus, String> {
    Ok(CapturePermissionStatus::unsupported(std::env::consts::OS))
}

#[tauri::command]
pub fn capture_permission_request() -> Result<CapturePermissionStatus, String> {
    capture_permission_status()
}

#[tauri::command]
pub fn capture_permission_open_settings() -> Result<(), String> {
    Err("screen capture is not enabled in this build".into())
}
