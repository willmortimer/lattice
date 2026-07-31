//! Cloud account bearer auth Tauri commands (ADR 0067).

use lattice_cloud_client::{CloudSessionStatus, PreferencesView};
use lattice_core::ensure_lattice_home;
use lattice_profile::{DesktopSettings, DESKTOP_SETTINGS_SPEC};
use serde_json::Value;

#[tauri::command]
pub fn cloud_session_status() -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_session_status_cmd()
}

#[tauri::command]
pub fn cloud_sign_in(email: String, password: String) -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_sign_in(email, password)
}

/// SIWA presents AppKit UI; keep it off Tauri's async worker by using a dedicated
/// blocking thread so the Swift bridge can wait without starving the main run loop
/// when the command happens to be polled on main.
#[tauri::command]
pub async fn cloud_sign_in_apple() -> Result<CloudSessionStatus, String> {
    tauri::async_runtime::spawn_blocking(lattice_handlers::cloud_sign_in_apple)
        .await
        .map_err(|err| format!("Sign in with Apple task failed: {err}"))?
}

#[tauri::command]
pub fn cloud_sign_out() -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_sign_out()
}

#[tauri::command]
pub fn cloud_update_preferences(
    ai_audit_enabled: Option<bool>,
    anonymous_telemetry_enabled: Option<bool>,
) -> Result<PreferencesView, String> {
    lattice_handlers::cloud_update_preferences(ai_audit_enabled, anonymous_telemetry_enabled)
}

#[tauri::command]
pub fn product_telemetry_emit(
    name: String,
    properties: Option<Value>,
) -> Result<(), String> {
    let enabled = ensure_lattice_home()
        .ok()
        .and_then(|home| {
            home.settings_store()
                .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
                .ok()
                .map(|loaded| loaded.value.privacy.anonymous_telemetry_enabled)
        })
        .unwrap_or(true);
    lattice_handlers::product_telemetry_emit(name, properties, enabled)
}
