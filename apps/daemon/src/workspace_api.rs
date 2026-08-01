//! HTTP/API helpers for the workspace-id registry (remote access lease).

use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::server::DaemonState;
use crate::workspace_registry::{
    sync_remote_access_lease, WorkspaceRegistry, WorkspaceRegistryRecord,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRemoteAccessParams {
    pub workspace_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRemoteAccessResponse {
    pub workspace: WorkspaceRegistryRecord,
    pub remote_access_lease_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRegistryListResponse {
    pub version: u32,
    pub workspaces: Vec<WorkspaceRegistryRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRemoteAccessListResponse {
    pub workspaces: Vec<WorkspaceRegistryRecord>,
    pub remote_access_lease_active: bool,
    /// True when `LATTICE_CLOUD_URL` / token / device id are set for the relay client.
    pub relay_configured: bool,
}

fn load_registry() -> Result<WorkspaceRegistry, ApiError> {
    WorkspaceRegistry::load_default().map_err(|err| ApiError::Internal(err.to_string()))
}

fn save_registry(registry: &WorkspaceRegistry) -> Result<(), ApiError> {
    registry
        .save_default()
        .map_err(|err| ApiError::Internal(err.to_string()))
}

fn relay_configured() -> bool {
    crate::cloud_relay::CloudRelayConfig::from_env().is_some()
}

async fn sync_lease(state: &DaemonState, registry: &WorkspaceRegistry) {
    if let Some(connections) = state.connections() {
        sync_remote_access_lease(connections, registry).await;
    }
}

/// List durable workspace-id registry entries (metadata only; no workspace open).
pub fn api_workspace_list_registry() -> Result<WorkspaceRegistryListResponse, ApiError> {
    let registry = load_registry()?;
    Ok(WorkspaceRegistryListResponse {
        version: registry.version,
        workspaces: registry.list().to_vec(),
    })
}

/// List registered workspaces and whether the remote-access idle lease is held.
pub async fn api_workspace_list_remote_access(
    state: &DaemonState,
) -> Result<WorkspaceRemoteAccessListResponse, ApiError> {
    let registry = load_registry()?;
    // Re-sync so the response matches the live ConnectionTracker after restarts.
    sync_lease(state, &registry).await;
    let lease_active = state
        .connections()
        .map(|tracker| tracker.remote_access_lease_held())
        .unwrap_or_else(|| registry.remote_access_any());
    Ok(WorkspaceRemoteAccessListResponse {
        workspaces: registry.list().to_vec(),
        remote_access_lease_active: lease_active,
        relay_configured: relay_configured(),
    })
}

/// Enable or disable remote MCP/relay access for a registered workspace.
pub async fn api_workspace_set_remote_access(
    state: &DaemonState,
    params: WorkspaceRemoteAccessParams,
) -> Result<WorkspaceRemoteAccessResponse, ApiError> {
    let workspace_id = params.workspace_id.trim();
    if workspace_id.is_empty() {
        return Err(ApiError::BadRequest("workspaceId is required".into()));
    }
    let mut registry = load_registry()?;
    if !registry.set_remote_access(workspace_id, params.enabled) {
        return Err(ApiError::NotFound(format!(
            "workspace not registered: {workspace_id}"
        )));
    }
    let workspace = registry
        .list()
        .iter()
        .find(|entry| entry.workspace_id == workspace_id)
        .cloned()
        .expect("workspace updated");
    save_registry(&registry)?;
    sync_lease(state, &registry).await;
    Ok(WorkspaceRemoteAccessResponse {
        workspace,
        remote_access_lease_active: registry.remote_access_any(),
    })
}
