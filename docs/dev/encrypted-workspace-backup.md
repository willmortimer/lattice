# Encrypted workspace backup (client)

Lattice Cloud exposes opaque backup storage at `PUT /v1/workspaces/{id}/backups`
(metadata at `GET /v1/workspaces/{id}/backups`, ciphertext at
`GET /v1/workspaces/{id}/backups/{backup_id}`). The server never decrypts
ciphertext; it only stores bytes and returns backup metadata (id, size,
`content_hash`). GET returns raw octets with `X-Lattice-Content-Hash` (lowercase
SHA-256 hex of the body).

## Client path — upload (S7)

1. Desktop builds a workspace backup payload (`LWBK` format in
   `lattice-workspace-crypto::build_workspace_backup_payload`).
2. Rust unlocks the workspace DEK via `workspace_crypto_unlock` (keychain-wrapped;
   DEK stays out of the webview).
3. `encrypt_blob` produces authenticated ciphertext under the DEK.
4. `lattice-cloud-client::put_workspace_backup` sends opaque bytes with
   `x-lattice-content-hash` (SHA-256 hex of ciphertext).
5. Settings → Features → Labs → **Encrypted backup to cloud** invokes the Tauri
   command `put_encrypted_workspace_backup_cmd`.

Cloud workspace rows are created on demand (`POST /v1/workspaces`) keyed by the
local manifest id (`local_workspace_id`).

## Client path — restore

1. Resolve the cloud workspace for the open local workspace (same
   `ensure_cloud_workspace` as upload).
2. Choose a backup: explicit `backup_id`, or latest from
   `list_workspace_backups` (server returns `created_at` DESC).
3. `lattice-cloud-client::get_workspace_backup` downloads ciphertext and verifies
   `X-Lattice-Content-Hash` when present.
4. Decrypt with the unlocked DEK; parse `LWBK` via
   `parse_workspace_backup_payload` (rejects `..` / absolute path escape).
5. Restore into a caller-chosen `target_root` (conflict-safe):
   - create directories as needed;
   - if a destination file exists with different bytes → skip and record
     `{ path, reason }` (never silent overwrite);
   - if missing or bytes equal → `atomic_write_file`;
   - write `lattice.yaml` from the payload manifest with the same rules.
6. Tauri command: `restore_encrypted_workspace_backup_cmd` with
   `{ root, targetRoot, backupId? }`. TypeScript helper:
   `restoreEncryptedWorkspaceBackup(root, targetRoot, workspaceId, backupId?)`.

## Client path — account-scoped restore (empty device)

Open-workspace list/restore calls `Workspace::open` and
`ensure_cloud_workspace`, which **creates** a cloud row when the local
manifest id is missing. An empty machine has no matching local id, so cold
restore cannot open Personal first.

Account-scoped commands take a **cloud workspace id**, never a local root, and
never call `ensure_cloud_workspace` / `POST /v1/workspaces`:

1. `list_account_cloud_workspaces` → `GET /v1/workspaces` and returns
   `id` / `name` / `localWorkspaceId` / `createdAt` (no secrets).
2. `list_encrypted_backups_for_cloud_workspace` → existing
   `list_workspace_backups` for that cloud id.
3. `restore_encrypted_backup_for_cloud_workspace` → GET ciphertext → unwrap
   `LWBE` with the account wrap key → `import_dek` for the payload workspace
   id → write `lattice.yaml` and files into `target_root`. Caller then
   `open_workspace`s the folder.

Fail-closed:

- Envelope present but unwrap fails → error; **do not** provision a DEK
  (`workspace_crypto_unlock` is not called).
- Legacy (non-`LWBE`) body → error on this path (no local DEK on a new
  device). Open-workspace restore still decrypts legacy ciphertext with the
  unlocked local DEK.

Tauri: `list_account_cloud_workspaces_cmd`,
`list_encrypted_backups_for_cloud_workspace_cmd`,
`restore_encrypted_backup_for_cloud_workspace_cmd`. TypeScript:
`listAccountCloudWorkspaces()`,
`listEncryptedBackupsForCloudWorkspace(cloudWorkspaceId)`,
`restoreEncryptedBackupForCloudWorkspace(cloudWorkspaceId, targetRoot, backupId)`.

The empty desktop shell (no open workspace) exposes **Sign in to Lattice Cloud**
and **Restore encrypted backup**. That path uses the account-scoped commands
above, then `open_workspace` on the chosen folder. It does not create Personal
or call `ensure_cloud_workspace`.

See also: ecosystem `lattice-cloud` `cloud_api.rs` backup handlers and
`docs/architecture/cloud-backend-dag.md` (CB2 opaque backup).
