//! Open-format workspace cloud sync Tauri commands.

use lattice_handlers::{
    push_pull_workspace_sync_for_root, resolve_workspace_sync_conflict, ExecuteResult,
    SyncRunReport,
};

/// Ensure the cloud workspace for this open root, then run one push/pull sync cycle.
#[tauri::command]
pub fn push_pull_workspace_sync_cmd(root: String) -> Result<SyncRunReport, String> {
    push_pull_workspace_sync_for_root(&root)
}

/// Resolve one conflicted resource: `keep_local` (push local) or `take_cloud` (pull cloud).
#[tauri::command]
pub fn resolve_workspace_sync_conflict_cmd(
    root: String,
    resource_id: String,
    resolution: String,
) -> Result<ExecuteResult, String> {
    resolve_workspace_sync_conflict(&root, &resource_id, &resolution)
}
