//! Workspace encryption + encrypted cloud backup Tauri commands.

use lattice_handlers::{
    put_encrypted_workspace_backup, restore_encrypted_workspace_backup, workspace_crypto_lock,
    workspace_crypto_status, workspace_crypto_unlock, EncryptedBackupPutResult,
    EncryptedBackupRestoreResult, WorkspaceCryptoStatus,
};

#[tauri::command]
pub fn workspace_crypto_status_cmd() -> WorkspaceCryptoStatus {
    workspace_crypto_status()
}

#[tauri::command]
pub fn workspace_crypto_unlock_cmd(workspace_id: String) -> Result<WorkspaceCryptoStatus, String> {
    workspace_crypto_unlock(workspace_id)
}

#[tauri::command]
pub fn workspace_crypto_lock_cmd() -> Result<WorkspaceCryptoStatus, String> {
    workspace_crypto_lock()
}

/// Build a workspace backup payload, encrypt with the unlocked DEK, and PUT opaque
/// ciphertext to `PUT /v1/workspaces/{id}/backups`. The webview never holds the DEK.
#[tauri::command]
pub fn put_encrypted_workspace_backup_cmd(
    root: String,
) -> Result<EncryptedBackupPutResult, String> {
    put_encrypted_workspace_backup(&root)
}

/// Download opaque ciphertext, decrypt with the unlocked DEK, and restore files into
/// `target_root` (conflict-safe: differing existing files are skipped).
#[tauri::command]
pub fn restore_encrypted_workspace_backup_cmd(
    root: String,
    target_root: String,
    backup_id: Option<String>,
) -> Result<EncryptedBackupRestoreResult, String> {
    restore_encrypted_workspace_backup(
        &root,
        &target_root,
        backup_id.as_deref(),
    )
}
