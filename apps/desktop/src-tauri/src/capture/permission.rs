//! Screen recording permission Tauri commands (feature `capture`).

use lattice_capture_core::{CapturePermissionProvider, CapturePermissionStatus};

use super::platform::platform_permission_provider;

#[tauri::command]
pub fn capture_permission_status() -> Result<CapturePermissionStatus, String> {
    Ok(platform_permission_provider().status())
}

#[tauri::command]
pub fn capture_permission_request() -> Result<CapturePermissionStatus, String> {
    Ok(platform_permission_provider().request())
}

#[tauri::command]
pub fn capture_permission_open_settings() -> Result<(), String> {
    platform_permission_provider().open_settings()
}
