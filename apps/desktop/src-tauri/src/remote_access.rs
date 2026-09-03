//! Remote-access / device-authority status for the desktop Settings panel.
//!
//! Prefers latticed HTTP (`/v1/workspace/list_remote_access`,
//! `/v1/workspace/set_remote_access`); falls back to the shared workspace
//! registry file so toggles persist when the daemon is offline.

use std::path::PathBuf;
use std::time::Duration;

use lattice_profile::{lattice_home_path, STATE_DIR_NAME};
use serde::{Deserialize, Serialize};

const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
const ENV_API_PORT: &str = "LATTICE_API_PORT";
const DEFAULT_API_PORT: u16 = 18787;
const REGISTRY_FILENAME: &str = "workspace-registry.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAccessWorkspace {
    pub workspace_id: String,
    pub root: String,
    pub remote_access_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAccessStatus {
    pub workspaces: Vec<RemoteAccessWorkspace>,
    pub remote_access_lease_active: bool,
    pub relay_configured: bool,
    pub daemon_reachable: bool,
    pub via: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryFile {
    #[serde(default)]
    workspaces: Vec<RegistryWorkspace>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegistryWorkspace {
    workspace_id: String,
    root: PathBuf,
    #[serde(default)]
    remote_access_enabled: bool,
}

fn registry_path() -> PathBuf {
    lattice_home_path()
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(lattice_profile::LATTICE_HOME_NAME)
        })
        .join(STATE_DIR_NAME)
        .join(REGISTRY_FILENAME)
}

fn relay_configured_from_env() -> bool {
    let url = std::env::var("LATTICE_CLOUD_URL").unwrap_or_default();
    let token = std::env::var("LATTICE_CLOUD_TOKEN").unwrap_or_default();
    let device = std::env::var("LATTICE_DEVICE_ID").unwrap_or_default();
    !url.trim().is_empty() && !token.trim().is_empty() && !device.trim().is_empty()
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

fn workspace_from_registry(entry: &RegistryWorkspace) -> RemoteAccessWorkspace {
    RemoteAccessWorkspace {
        workspace_id: entry.workspace_id.clone(),
        root: entry.root.to_string_lossy().replace('\\', "/"),
        remote_access_enabled: entry.remote_access_enabled,
    }
}

fn status_from_registry(
    registry: &RegistryFile,
    via: &str,
    daemon_reachable: bool,
) -> RemoteAccessStatus {
    let workspaces: Vec<RemoteAccessWorkspace> = registry
        .workspaces
        .iter()
        .map(workspace_from_registry)
        .collect();
    let lease = workspaces.iter().any(|w| w.remote_access_enabled);
    RemoteAccessStatus {
        workspaces,
        remote_access_lease_active: lease,
        relay_configured: relay_configured_from_env(),
        daemon_reachable,
        via: via.to_string(),
    }
}

fn load_registry_file() -> Result<RegistryFile, String> {
    let path = registry_path();
    if !path.is_file() {
        return Ok(RegistryFile {
            workspaces: Vec::new(),
        });
    }
    let raw = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    if raw.trim().is_empty() {
        return Ok(RegistryFile {
            workspaces: Vec::new(),
        });
    }
    serde_json::from_str(&raw).map_err(|err| err.to_string())
}

fn save_registry_file(registry: &RegistryFile) -> Result<(), String> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let body = serde_json::json!({
        "version": 1,
        "workspaces": registry.workspaces,
    });
    let text = serde_json::to_string_pretty(&body).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{text}\n")).map_err(|err| err.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
    Ok(())
}

fn status_from_http(body: serde_json::Value) -> Option<RemoteAccessStatus> {
    let workspaces_value = body.get("workspaces")?.clone();
    let entries: Vec<RegistryWorkspace> = serde_json::from_value(workspaces_value).ok()?;
    let mut status = status_from_registry(
        &RegistryFile {
            workspaces: entries,
        },
        "http",
        true,
    );
    if let Some(active) = body
        .get("remoteAccessLeaseActive")
        .and_then(|value| value.as_bool())
    {
        status.remote_access_lease_active = active;
    }
    if let Some(configured) = body
        .get("relayConfigured")
        .and_then(|value| value.as_bool())
    {
        status.relay_configured = configured;
    }
    Some(status)
}

#[tauri::command]
pub fn get_remote_access_status() -> Result<RemoteAccessStatus, String> {
    if let Some(body) = http_post_json("/v1/workspace/list_remote_access", &serde_json::json!({})) {
        if let Some(status) = status_from_http(body) {
            return Ok(status);
        }
    }
    let registry = load_registry_file()?;
    Ok(status_from_registry(&registry, "file", false))
}

#[tauri::command]
pub fn set_workspace_remote_access(
    workspace_id: String,
    enabled: bool,
) -> Result<RemoteAccessStatus, String> {
    let workspace_id = workspace_id.trim().to_string();
    if workspace_id.is_empty() {
        return Err("workspaceId is required".into());
    }

    if let Some(body) = http_post_json(
        "/v1/workspace/set_remote_access",
        &serde_json::json!({
            "workspaceId": workspace_id,
            "enabled": enabled,
        }),
    ) {
        if body.get("workspace").is_some() {
            return get_remote_access_status();
        }
        if let Some(message) = body.get("error").and_then(|value| value.as_str()) {
            return Err(message.to_string());
        }
        if let Some(message) = body.get("message").and_then(|value| value.as_str()) {
            return Err(message.to_string());
        }
    }

    let mut registry = load_registry_file()?;
    let Some(entry) = registry
        .workspaces
        .iter_mut()
        .find(|entry| entry.workspace_id == workspace_id)
    else {
        return Err(format!("workspace not registered: {workspace_id}"));
    };
    entry.remote_access_enabled = enabled;
    save_registry_file(&registry)?;
    Ok(status_from_registry(&registry, "file", false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_from_registry_computes_lease() {
        let registry = RegistryFile {
            workspaces: vec![
                RegistryWorkspace {
                    workspace_id: "ws-a".into(),
                    root: PathBuf::from("/tmp/a"),
                    remote_access_enabled: false,
                },
                RegistryWorkspace {
                    workspace_id: "ws-b".into(),
                    root: PathBuf::from("/tmp/b"),
                    remote_access_enabled: true,
                },
            ],
        };
        let status = status_from_registry(&registry, "file", false);
        assert!(status.remote_access_lease_active);
        assert_eq!(status.workspaces.len(), 2);
        assert_eq!(status.via, "file");
    }

    #[test]
    fn workspace_root_normalizes_separators() {
        let entry = RegistryWorkspace {
            workspace_id: "ws".into(),
            root: PathBuf::from(r"C:\Users\demo"),
            remote_access_enabled: true,
        };
        let mapped = workspace_from_registry(&entry);
        assert!(!mapped.root.contains('\\'));
        assert!(mapped.remote_access_enabled);
    }
}
