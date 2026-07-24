//! HTTP/API helpers for the known-workspace schedule registry.

use std::path::PathBuf;

use lattice_profile::{
    default_scheduler_registry_path, KnownWorkspaceEntry, KnownWorkspaceRegistry,
};
use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::server::DaemonState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerWorkspaceParams {
    pub root: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerSetEnabledParams {
    pub root: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerWorkspaceResponse {
    pub workspace: KnownWorkspaceEntry,
    pub scheduler_lease_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerListResponse {
    pub workspaces: Vec<KnownWorkspaceEntry>,
    pub scheduler_lease_active: bool,
    pub registry_path: String,
}

fn registry_path() -> PathBuf {
    default_scheduler_registry_path()
}

fn load_registry() -> Result<KnownWorkspaceRegistry, ApiError> {
    KnownWorkspaceRegistry::load_or_default(&registry_path())
        .map_err(|err| ApiError::Internal(err.to_string()))
}

fn save_registry(registry: &KnownWorkspaceRegistry) -> Result<(), ApiError> {
    registry
        .save(&registry_path())
        .map_err(|err| ApiError::Internal(err.to_string()))
}

fn parse_root(root: &str) -> Result<PathBuf, ApiError> {
    let trimmed = root.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest("root is required".into()));
    }
    Ok(PathBuf::from(trimmed))
}

async fn sync_lease(state: &DaemonState, registry: &KnownWorkspaceRegistry) {
    if let Some(connections) = state.connections() {
        connections
            .set_scheduler_lease(registry.scheduler_lease_active())
            .await;
    }
}

/// Register a workspace for background interval schedules.
pub async fn api_scheduler_register(
    state: &DaemonState,
    params: SchedulerWorkspaceParams,
) -> Result<SchedulerWorkspaceResponse, ApiError> {
    let root = parse_root(&params.root)?;
    let enabled = params.enabled.unwrap_or(true);
    let mut registry = load_registry()?;
    let entry = registry.register(&root, enabled).clone();
    save_registry(&registry)?;
    sync_lease(state, &registry).await;
    Ok(SchedulerWorkspaceResponse {
        workspace: entry,
        scheduler_lease_active: registry.scheduler_lease_active(),
    })
}

/// Remove a workspace from the known-workspace registry.
pub async fn api_scheduler_unregister(
    state: &DaemonState,
    params: SchedulerWorkspaceParams,
) -> Result<SchedulerListResponse, ApiError> {
    let root = parse_root(&params.root)?;
    let mut registry = load_registry()?;
    let _ = registry.unregister(&root);
    save_registry(&registry)?;
    sync_lease(state, &registry).await;
    Ok(list_response(&registry))
}

/// Enable or disable schedules for a registered workspace.
pub async fn api_scheduler_set_enabled(
    state: &DaemonState,
    params: SchedulerSetEnabledParams,
) -> Result<SchedulerWorkspaceResponse, ApiError> {
    let root = parse_root(&params.root)?;
    let mut registry = load_registry()?;
    let entry = match registry.set_enabled(&root, params.enabled) {
        Some(entry) => entry.clone(),
        None => {
            // Opt-in convenience: enabling an unknown root registers it.
            if params.enabled {
                registry.register(&root, true).clone()
            } else {
                return Err(ApiError::NotFound(format!(
                    "workspace not registered: {}",
                    root.display()
                )));
            }
        }
    };
    save_registry(&registry)?;
    sync_lease(state, &registry).await;
    Ok(SchedulerWorkspaceResponse {
        workspace: entry,
        scheduler_lease_active: registry.scheduler_lease_active(),
    })
}

/// List registered workspaces and current scheduler lease intent.
pub async fn api_scheduler_list(state: &DaemonState) -> Result<SchedulerListResponse, ApiError> {
    let registry = load_registry()?;
    sync_lease(state, &registry).await;
    Ok(list_response(&registry))
}

fn list_response(registry: &KnownWorkspaceRegistry) -> SchedulerListResponse {
    SchedulerListResponse {
        workspaces: registry.workspaces.clone(),
        scheduler_lease_active: registry.scheduler_lease_active(),
        registry_path: registry_path().to_string_lossy().replace('\\', "/"),
    }
}
