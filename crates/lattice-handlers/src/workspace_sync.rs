//! Workspace blob push/pull via cloud sync-heads + planner executor.

use std::path::Path;

use lattice_cloud_client::{default_client, CloudApiClient, CloudHttpClient, HttpCloudClient};
use lattice_core::Workspace;
use lattice_sync::{ConflictResolution, ExecuteResult, SyncRunReport};

use crate::cloud::resolve_cloud_bearer_cmd;
use crate::workspace_backup::ensure_cloud_workspace;

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn api_client() -> CloudApiClient<HttpCloudClient> {
    default_client()
}

fn parse_resolution(resolution: &str) -> Result<ConflictResolution, String> {
    match resolution.trim() {
        "keep_local" => Ok(ConflictResolution::KeepLocal),
        "take_cloud" => Ok(ConflictResolution::TakeCloud),
        other => Err(format!(
            "invalid conflict resolution '{other}' (expected keep_local or take_cloud)"
        )),
    }
}

/// Run one push/pull sync cycle for an open workspace root.
pub fn push_pull_workspace_sync(
    root: &str,
    cloud_workspace_id: &str,
) -> Result<SyncRunReport, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    push_pull_workspace_sync_with_client(&api_client(), &bearer, root, cloud_workspace_id)
}

pub fn push_pull_workspace_sync_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    cloud_workspace_id: &str,
) -> Result<SyncRunReport, String> {
    lattice_sync::run_workspace_sync(client, Path::new(root), cloud_workspace_id, bearer)
        .map_err(map_err)
}

/// Ensure a cloud workspace exists for the open root, then run push/pull sync.
pub fn push_pull_workspace_sync_for_root(root: &str) -> Result<SyncRunReport, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    push_pull_workspace_sync_for_root_with_client(&api_client(), &bearer, root)
}

pub fn push_pull_workspace_sync_for_root_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
) -> Result<SyncRunReport, String> {
    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let local_id = manifest.id.to_string();
    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &local_id, manifest.title.as_str())?;
    push_pull_workspace_sync_with_client(client, bearer, root, &cloud_workspace.id)
}

/// Resolve one conflicted resource: `keep_local` or `take_cloud`.
pub fn resolve_workspace_sync_conflict(
    root: &str,
    resource_id: &str,
    resolution: &str,
) -> Result<ExecuteResult, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    resolve_workspace_sync_conflict_with_client(
        &api_client(),
        &bearer,
        root,
        resource_id,
        resolution,
    )
}

pub fn resolve_workspace_sync_conflict_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    resource_id: &str,
    resolution: &str,
) -> Result<ExecuteResult, String> {
    let resolution = parse_resolution(resolution)?;
    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let local_id = manifest.id.to_string();
    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &local_id, manifest.title.as_str())?;
    lattice_sync::resolve_conflict(
        client,
        Path::new(root),
        &cloud_workspace.id,
        bearer,
        resource_id,
        resolution,
    )
    .map_err(map_err)
}
