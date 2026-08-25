//! Workspace encryption + encrypted cloud backup Tauri commands.

use lattice_handlers::{
    list_account_cloud_workspaces, list_encrypted_backups_for_cloud_workspace,
    list_encrypted_workspace_backups, put_encrypted_workspace_backup,
    restore_encrypted_backup_for_cloud_workspace, restore_encrypted_workspace_backup,
    workspace_crypto_lock, workspace_crypto_status, workspace_crypto_unlock,
    AccountCloudWorkspaceEntry, EncryptedBackupListEntry, EncryptedBackupPutResult,
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

/// List encrypted workspace backup metadata (no DEK unlock; no object keys).
#[tauri::command]
pub fn list_encrypted_workspace_backups_cmd(
    root: String,
) -> Result<Vec<EncryptedBackupListEntry>, String> {
    list_encrypted_workspace_backups(&root)
}

/// Download opaque ciphertext, decrypt with the unlocked DEK, and restore files into
/// `target_root` (conflict-safe: differing existing files are skipped).
#[tauri::command]
pub fn restore_encrypted_workspace_backup_cmd(
    root: String,
    target_root: String,
    backup_id: Option<String>,
) -> Result<EncryptedBackupRestoreResult, String> {
    restore_encrypted_workspace_backup(&root, &target_root, backup_id.as_deref())
}

/// List cloud workspace rows for the signed-in account (no secrets; no create).
#[tauri::command]
pub fn list_account_cloud_workspaces_cmd() -> Result<Vec<AccountCloudWorkspaceEntry>, String> {
    list_account_cloud_workspaces()
}

/// List encrypted backups for a cloud workspace id without opening a local root.
#[tauri::command]
pub fn list_encrypted_backups_for_cloud_workspace_cmd(
    cloud_workspace_id: String,
) -> Result<Vec<EncryptedBackupListEntry>, String> {
    list_encrypted_backups_for_cloud_workspace(&cloud_workspace_id)
}

/// Restore an LWBE backup into `target_root` using a cloud workspace id.
///
/// Does not unlock/provision a local DEK; envelope unwrap imports the backed-up key.
#[tauri::command]
pub fn restore_encrypted_backup_for_cloud_workspace_cmd(
    cloud_workspace_id: String,
    target_root: String,
    backup_id: String,
) -> Result<EncryptedBackupRestoreResult, String> {
    restore_encrypted_backup_for_cloud_workspace(&cloud_workspace_id, &target_root, &backup_id)
}
