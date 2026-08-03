//! Cloud account bearer auth + Labs cloud blob Tauri commands (ADR 0067).

use lattice_cloud_client::{
    default_client, CloudSessionStatus, HttpCloudBlobClient, PreferencesView,
};
use lattice_core::ensure_lattice_home;
use lattice_profile::{DesktopSettings, DESKTOP_SETTINGS_SPEC};
use latticefs_core::{
    materialize_to_cloud, open_cloud_authoritative_bytes, CloudBlobClient, ResourceStat,
};
use serde_json::Value;

use crate::commands::resolve_within_root;

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

/// Open browser SIWA for Developer ID / Windows builds (no native SIWA entitlement).
#[tauri::command]
pub fn cloud_begin_browser_siwa(app_base_url: Option<String>) -> Result<String, String> {
    lattice_handlers::cloud_begin_browser_siwa(app_base_url)
}

#[tauri::command]
pub fn cloud_complete_desktop_handoff(
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
) -> Result<CloudSessionStatus, String> {
    lattice_handlers::cloud_complete_desktop_handoff(code, state, error)
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

/// Fail closed: Labs cloud blob ops require a signed-in bearer (or `LATTICE_CLOUD_TOKEN`).
fn require_cloud_blob_bearer(token: Result<String, String>) -> Result<String, String> {
    token
}

/// PUT→GET verify via [`HttpCloudBlobClient`], then set registry authority to cloud.
#[tauri::command]
pub fn cloud_blob_materialize(root: String, rel_path: String) -> Result<ResourceStat, String> {
    let token = require_cloud_blob_bearer(lattice_handlers::resolve_cloud_bearer_cmd())?;
    let client = HttpCloudBlobClient::new(default_client(), token);
    cloud_blob_materialize_with_client(&root, &rel_path, &client)
}

/// GET canonical cloud bytes for a cloud-authoritative path (no local fallback).
#[tauri::command]
pub fn cloud_blob_open(root: String, rel_path: String) -> Result<Vec<u8>, String> {
    let token = require_cloud_blob_bearer(lattice_handlers::resolve_cloud_bearer_cmd())?;
    let client = HttpCloudBlobClient::new(default_client(), token);
    cloud_blob_open_with_client(&root, &rel_path, &client)
}

fn cloud_blob_materialize_with_client(
    root: &str,
    rel_path: &str,
    client: &dyn CloudBlobClient,
) -> Result<ResourceStat, String> {
    let (canonical_root, file_path) = resolve_within_root(root, rel_path)?;
    let rel_key = rel_path.replace('\\', "/");
    let data = std::fs::read(&file_path).map_err(|err| err.to_string())?;
    materialize_to_cloud(&canonical_root, &rel_key, &data, client).map_err(|err| err.to_string())
}

fn cloud_blob_open_with_client(
    root: &str,
    rel_path: &str,
    client: &dyn CloudBlobClient,
) -> Result<Vec<u8>, String> {
    let (canonical_root, _) = resolve_within_root(root, rel_path)?;
    let rel_key = rel_path.replace('\\', "/");
    open_cloud_authoritative_bytes(&canonical_root, &rel_key, client).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use latticefs_core::{AuthorityMode, InMemoryCloudBlobClient};

    #[test]
    fn cloud_blob_ops_fail_closed_when_unsigned() {
        let err = require_cloud_blob_bearer(Err(
            "not signed in to cloud; sign in via desktop Settings → Cloud account, \
             or set LATTICE_CLOUD_TOKEN"
                .into(),
        ))
        .unwrap_err();
        assert!(
            err.contains("not signed in"),
            "expected clear unsigned error, got: {err}"
        );
    }

    #[test]
    fn materialize_with_in_memory_sets_cloud_authority() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "LabsCloud").unwrap();
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        let rel = "notes/lab.md";
        let data = b"labs-cloud-roundtrip";
        std::fs::write(dir.path().join(rel), data).unwrap();

        let client = InMemoryCloudBlobClient::new();
        let root = dir.path().to_string_lossy().into_owned();
        let stat = cloud_blob_materialize_with_client(&root, rel, &client).unwrap();
        assert_eq!(stat.authority, AuthorityMode::Cloud);
        assert_eq!(stat.path, rel);

        let reopened = cloud_blob_open_with_client(&root, rel, &client).unwrap();
        assert_eq!(reopened, data);
    }

    #[test]
    fn open_fails_when_authority_still_local() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "LabsCloud").unwrap();
        let rel = "local-only.md";
        std::fs::write(dir.path().join(rel), b"still-local").unwrap();

        // Register as local without materializing to cloud.
        let _ = latticefs_core::resource_stat_or_register(dir.path(), rel).unwrap();

        let client = InMemoryCloudBlobClient::new();
        let root = dir.path().to_string_lossy().into_owned();
        let err = cloud_blob_open_with_client(&root, rel, &client).unwrap_err();
        assert!(
            err.to_lowercase().contains("cloud") || err.to_lowercase().contains("authority"),
            "expected not-cloud-authoritative error, got: {err}"
        );
    }
}
