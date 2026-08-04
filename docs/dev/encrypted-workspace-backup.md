# Encrypted workspace backup (client)

Lattice Cloud exposes opaque backup storage at `PUT /v1/workspaces/{id}/backups`
(metadata at `GET /v1/workspaces/{id}/backups`). The server never decrypts
ciphertext; it only stores bytes and returns backup metadata (id, size,
`content_hash`).

## Client path (S7)

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

See also: ecosystem `lattice-cloud` `cloud_api.rs` backup handlers and
`docs/architecture/cloud-backend-dag.md` (CB2 opaque backup).
