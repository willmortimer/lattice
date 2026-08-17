//! Cloud Yrs remote snapshot and append-log Tauri commands (S8).

use lattice_handlers::{
    pull_collab_remote_log, pull_collab_remote_snapshot, push_collab_remote_log,
    push_collab_remote_snapshot, replace_collab_remote_log, CollabRemoteLogPullResult,
    CollabRemoteLogPushResult, CollabRemotePullResult, CollabRemotePushResult,
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

/// Append one lib0 v1 update to the cloud LYRL sidecar for `doc_id`.
///
/// `base_hash` is the 64-character hex SHA-256 of the LYRS snapshot this log is
/// based on. Omit or pass empty to use 32 zero bytes (`REMOTE_LOG_UNKNOWN_BASE_HASH`).
/// Raw 32-byte digests should be hex-encoded before invoke.
#[tauri::command]
pub fn push_collab_remote_log_cmd(
    root: String,
    doc_id: String,
    update: Vec<u8>,
    base_hash: Option<String>,
) -> Result<CollabRemoteLogPushResult, String> {
    push_collab_remote_log(
        &root,
        &doc_id,
        &update,
        base_hash.as_deref().map(str::as_bytes),
    )
}

/// GET the cloud LYRL sidecar for `doc_id`, if any.
///
/// `baseHash` in the result is 32 raw SHA-256 bytes (not hex). Hex-encode it
/// if passing the value back into [`push_collab_remote_log_cmd`].
#[tauri::command]
pub fn pull_collab_remote_log_cmd(
    root: String,
    doc_id: String,
) -> Result<Option<CollabRemoteLogPullResult>, String> {
    pull_collab_remote_log(&root, &doc_id)
}

/// Replace the cloud LYRL sidecar for `doc_id` (no append).
///
/// `base_hash` is the 64-character hex SHA-256 of the LYRS snapshot this log is
/// based on. Omit or pass empty for [`REMOTE_LOG_UNKNOWN_BASE_HASH`].
/// `updates` may be empty after compaction.
#[tauri::command]
pub fn replace_collab_remote_log_cmd(
    root: String,
    doc_id: String,
    base_hash: Option<String>,
    updates: Vec<Vec<u8>>,
) -> Result<CollabRemoteLogPushResult, String> {
    replace_collab_remote_log(
        &root,
        &doc_id,
        base_hash.as_deref().map(str::as_bytes),
        &updates,
    )
}
