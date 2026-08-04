//! Open-format workspace cloud sync Tauri commands.

use lattice_handlers::{push_pull_workspace_sync_for_root, SyncRunReport};

/// Ensure the cloud workspace for this open root, then run one push/pull sync cycle.
#[tauri::command]
pub fn push_pull_workspace_sync_cmd(root: String) -> Result<SyncRunReport, String> {
    push_pull_workspace_sync_for_root(&root)
}
