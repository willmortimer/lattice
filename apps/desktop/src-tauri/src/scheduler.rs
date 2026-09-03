//! Known-workspace background schedule registration for the desktop shell.
//!
//! Prefers the latticed HTTP scheduler API when available; otherwise writes the
//! shared registry file directly so opt-in persists for the next daemon tick.

use std::path::Path;
use std::time::Duration;

use lattice_profile::{
    default_scheduler_registry_path, KnownWorkspaceEntry, KnownWorkspaceRegistry,
};
use serde::Serialize;

const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
const ENV_API_PORT: &str = "LATTICE_API_PORT";
const DEFAULT_API_PORT: u16 = 18787;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundScheduleStatus {
    pub root: String,
    pub registered: bool,
    pub enabled: bool,
    pub scheduler_lease_active: bool,
    pub last_error: Option<String>,
    pub schedule_workflows: Vec<String>,
    pub via: String,
}

fn normalize_root(root: &str) -> String {
    let path = Path::new(root);
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn entry_for_root<'a>(
    registry: &'a KnownWorkspaceRegistry,
    root: &str,
) -> Option<&'a KnownWorkspaceEntry> {
    let key = normalize_root(root);
    registry
        .workspaces
        .iter()
        .find(|entry| entry.root == key || entry.root == root)
}

fn status_from_registry(
    root: &str,
    registry: &KnownWorkspaceRegistry,
    via: &str,
) -> BackgroundScheduleStatus {
    let entry = entry_for_root(registry, root);
    BackgroundScheduleStatus {
        root: normalize_root(root),
        registered: entry.is_some(),
        enabled: entry.map(|e| e.enabled).unwrap_or(false),
        scheduler_lease_active: registry.scheduler_lease_active(),
        last_error: entry.and_then(|e| e.last_error.clone()),
        schedule_workflows: entry
            .map(|e| e.schedule_workflows.clone())
            .unwrap_or_default(),
        via: via.to_string(),
    }
}

fn http_base() -> Option<(String, u16)> {
    let token = std::env::var(ENV_AUTH_TOKEN).ok()?;
    if token.trim().is_empty() {
        return None;
    }
    let port = std::env::var(ENV_API_PORT)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_API_PORT);
    Some((token, port))
}

fn http_post_json(path: &str, body: &serde_json::Value) -> Option<serde_json::Value> {
    let (token, port) = http_base()?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let response = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("content-type", "application/json")
        .timeout(Duration::from_millis(800))
        .send_string(&body.to_string())
        .ok()?;
    let text = response.into_string().ok()?;
    serde_json::from_str(&text).ok()
}

#[tauri::command]
pub fn get_background_schedule_status(root: String) -> Result<BackgroundScheduleStatus, String> {
    if root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    if let Some(body) = http_post_json("/v1/scheduler/list", &serde_json::json!({})) {
        if let Ok(registry) = serde_json::from_value::<KnownWorkspaceRegistry>(serde_json::json!({
            "version": 1,
            "workspaces": body.get("workspaces").cloned().unwrap_or(serde_json::json!([])),
        })) {
            let mut status = status_from_registry(&root, &registry, "http");
            if let Some(active) = body.get("schedulerLeaseActive").and_then(|v| v.as_bool()) {
                status.scheduler_lease_active = active;
            }
            return Ok(status);
        }
    }

    let registry = KnownWorkspaceRegistry::load_or_default(&default_scheduler_registry_path())
        .map_err(|err| err.to_string())?;
    Ok(status_from_registry(&root, &registry, "file"))
}

#[tauri::command]
pub fn set_background_schedules_enabled(
    root: String,
    enabled: bool,
) -> Result<BackgroundScheduleStatus, String> {
    if root.trim().is_empty() {
        return Err("workspace root is required".into());
    }

    if enabled {
        if let Some(body) = http_post_json(
            "/v1/scheduler/set_enabled",
            &serde_json::json!({ "root": root, "enabled": true }),
        ) {
            if body.get("workspace").is_some() {
                return get_background_schedule_status(root);
            }
        }
        let path = default_scheduler_registry_path();
        let mut registry =
            KnownWorkspaceRegistry::load_or_default(&path).map_err(|e| e.to_string())?;
        registry.register(Path::new(&root), true);
        registry.save(&path).map_err(|e| e.to_string())?;
        return Ok(status_from_registry(&root, &registry, "file"));
    }

    if let Some(body) = http_post_json(
        "/v1/scheduler/set_enabled",
        &serde_json::json!({ "root": root, "enabled": false }),
    ) {
        if body.get("workspace").is_some() || body.get("error").is_none() {
            return get_background_schedule_status(root);
        }
    }
    // Fall back: disable or unregister via file.
    let path = default_scheduler_registry_path();
    let mut registry = KnownWorkspaceRegistry::load_or_default(&path).map_err(|e| e.to_string())?;
    if registry.set_enabled(Path::new(&root), false).is_none() {
        let _ = registry.unregister(Path::new(&root));
    }
    registry.save(&path).map_err(|e| e.to_string())?;
    Ok(status_from_registry(&root, &registry, "file"))
}
