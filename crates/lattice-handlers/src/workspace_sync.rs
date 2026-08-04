//! Workspace blob push/pull via cloud sync-heads + planner executor.

use std::path::Path;

use lattice_cloud_client::{default_client, CloudApiClient, CloudHttpClient, HttpCloudClient};
use lattice_sync::SyncRunReport;

use crate::cloud::resolve_cloud_bearer_cmd;

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn api_client() -> CloudApiClient<HttpCloudClient> {
    default_client()
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
    lattice_sync::run_workspace_sync(
        client,
        Path::new(root),
        cloud_workspace_id,
        bearer,
    )
    .map_err(map_err)
}
