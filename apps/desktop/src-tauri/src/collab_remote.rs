//! Cloud Yrs remote snapshot Tauri commands (S8).

use lattice_handlers::{
    pull_collab_remote_snapshot, push_collab_remote_snapshot, CollabRemotePullResult,
    CollabRemotePushResult,
};

/// PUT a full Yrs update to the cloud sidecar blob for `doc_id`.
#[tauri::command]
pub fn push_collab_remote_snapshot_cmd(
    root: String,
    doc_id: String,
    update: Vec<u8>,
    if_match: Option<String>,
) -> Result<CollabRemotePushResult, String> {
    push_collab_remote_snapshot(&root, &doc_id, &update, if_match.as_deref())
}

/// GET the cloud sidecar Yrs snapshot for `doc_id`, if any.
#[tauri::command]
pub fn pull_collab_remote_snapshot_cmd(
    root: String,
    doc_id: String,
) -> Result<Option<CollabRemotePullResult>, String> {
    pull_collab_remote_snapshot(&root, &doc_id)
}
