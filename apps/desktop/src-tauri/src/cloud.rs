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
