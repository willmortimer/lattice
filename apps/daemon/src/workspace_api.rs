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

fn load_registry() -> Result<WorkspaceRegistry, ApiError> {
    WorkspaceRegistry::load_default().map_err(|err| ApiError::Internal(err.to_string()))
}

fn save_registry(registry: &WorkspaceRegistry) -> Result<(), ApiError> {
    registry
        .save_default()
        .map_err(|err| ApiError::Internal(err.to_string()))
}

async fn sync_lease(state: &DaemonState, registry: &WorkspaceRegistry) {
    if let Some(connections) = state.connections() {
        sync_remote_access_lease(connections, registry).await;
    }
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
