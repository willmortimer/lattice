//! Cloud account bearer auth Tauri commands (ADR 0067).

use lattice_cloud_client::CloudSessionStatus;

#[tauri::command]
pub fn cloud_session_status() -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_session_status_cmd()
}

#[tauri::command]
pub fn cloud_sign_in(email: String, password: String) -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_sign_in(email, password)
}

#[tauri::command]
pub fn cloud_sign_in_apple() -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_sign_in_apple()
}

#[tauri::command]
pub fn cloud_sign_out() -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_sign_out()
}
