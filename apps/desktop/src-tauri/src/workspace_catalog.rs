//! Workspace catalog for the multi-workspace Home/switcher (ADR 0079).
//!
//! Lists durable workspace-registry entries without opening workspaces.
//! Summary reads `lattice.yaml` head only — no resource scan.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lattice_core::{WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME};
use lattice_profile::{lattice_home_path, STATE_DIR_NAME};
use serde::{Deserialize, Serialize};

const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
const ENV_API_PORT: &str = "LATTICE_API_PORT";
const DEFAULT_API_PORT: u16 = 18787;
const REGISTRY_FILENAME: &str = "workspace-registry.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCatalogEntry {
    pub workspace_id: String,
    pub root: String,
    pub remote_access_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<WorkspaceCatalogEntry>,
    pub daemon_reachable: bool,
    pub via: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub workspace_id: String,
    pub root: String,
    pub title: String,
    pub remote_access_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_template: Option<String>,
    pub manifest_present: bool,
    pub via: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryFile {
    #[serde(default)]
    workspaces: Vec<RegistryWorkspace>,
}

#[derive(Debug, Clone, Deserialize)]
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

fn normalize_root_path(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn catalog_entry_from_registry(entry: &RegistryWorkspace) -> WorkspaceCatalogEntry {
    WorkspaceCatalogEntry {
        workspace_id: entry.workspace_id.clone(),
        root: normalize_root_path(&entry.root),
        remote_access_enabled: entry.remote_access_enabled,
    }
}

fn catalog_from_registry(registry: &RegistryFile, via: &str, daemon_reachable: bool) -> WorkspaceCatalog {
    WorkspaceCatalog {
        workspaces: registry
            .workspaces
            .iter()
            .map(catalog_entry_from_registry)
            .collect(),
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

fn title_from_root(root: &str) -> String {
    let normalized = root.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|part| !part.is_empty()).collect();
    parts
        .last()
        .map(|segment| (*segment).to_string())
        .unwrap_or_else(|| "Workspace".into())
}

fn manifest_head(root: &Path) -> Option<WorkspaceManifest> {
    let manifest_file = root.join(WORKSPACE_MANIFEST_FILENAME);
    if !manifest_file.is_file() {
        return None;
    }
    WorkspaceManifest::load(&manifest_file).ok()
}

fn summary_from_registry_entry(entry: &RegistryWorkspace, via: &str) -> WorkspaceSummary {
    let root = normalize_root_path(&entry.root);
    let manifest = manifest_head(Path::new(&root));
    let manifest_present = manifest.is_some();
    let title = manifest
        .as_ref()
        .map(|head| head.title.clone())
        .unwrap_or_else(|| title_from_root(&root));
    WorkspaceSummary {
        workspace_id: entry.workspace_id.clone(),
        root,
        title,
        remote_access_enabled: entry.remote_access_enabled,
        source_template: manifest.and_then(|head| head.source_template),
        manifest_present,
        via: via.to_string(),
    }
}

fn load_registry_with_via() -> Result<(RegistryFile, String, bool), String> {
    if let Some(body) = http_post_json("/v1/workspace/list_registry", &serde_json::json!({})) {
        if let Some(workspaces_value) = body.get("workspaces") {
            if let Ok(entries) = serde_json::from_value::<Vec<RegistryWorkspace>>(workspaces_value.clone())
            {
                return Ok((
                    RegistryFile {
                        workspaces: entries,
                    },
                    "http".to_string(),
                    true,
                ));
            }
        }
    }
    let registry = load_registry_file()?;
    Ok((registry, "file".to_string(), false))
}

fn load_catalog() -> Result<WorkspaceCatalog, String> {
    let (registry, via, daemon_reachable) = load_registry_with_via()?;
    Ok(catalog_from_registry(&registry, &via, daemon_reachable))
}

#[tauri::command]
pub fn list_workspace_catalog() -> Result<WorkspaceCatalog, String> {
    load_catalog()
}

#[tauri::command]
pub fn get_workspace_summary(workspace_id: String) -> Result<WorkspaceSummary, String> {
    let workspace_id = workspace_id.trim().to_string();
    if workspace_id.is_empty() {
        return Err("workspaceId is required".into());
    }

    let (registry, via, _) = load_registry_with_via()?;
    let entry = registry
        .workspaces
        .iter()
        .find(|entry| entry.workspace_id == workspace_id)
        .ok_or_else(|| format!("workspace not registered: {workspace_id}"))?;

    Ok(summary_from_registry_entry(entry, &via))
}

/// Resolve a registered workspace id to its root path for `open_workspace`.
pub fn resolve_workspace_root(workspace_id: &str) -> Result<String, String> {
    let workspace_id = workspace_id.trim();
    if workspace_id.is_empty() {
        return Err("workspaceId is required".into());
    }
    let (registry, _, _) = load_registry_with_via()?;
    let entry = registry
        .workspaces
        .iter()
        .find(|entry| entry.workspace_id == workspace_id)
        .ok_or_else(|| format!("workspace not registered: {workspace_id}"))?;
    Ok(normalize_root_path(&entry.root))
}

#[tauri::command]
pub fn open_workspace_by_id(workspace_id: String) -> Result<String, String> {
    resolve_workspace_root(&workspace_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_from_registry_maps_entries() {
        let registry = RegistryFile {
            workspaces: vec![RegistryWorkspace {
                workspace_id: "ws-a".into(),
                root: PathBuf::from("/tmp/demo"),
                remote_access_enabled: true,
            }],
        };
        let catalog = catalog_from_registry(&registry, "file", false);
        assert_eq!(catalog.workspaces.len(), 1);
        assert!(catalog.workspaces[0].remote_access_enabled);
        assert_eq!(catalog.workspaces[0].root, "/tmp/demo");
    }

    #[test]
    fn title_from_root_uses_last_segment() {
        assert_eq!(title_from_root("/Users/demo/Notes"), "Notes");
    }
}
