//! Encrypted workspace backup PUT/GET restore (opaque ciphertext via lattice-server).

use std::fs;
use std::path::{Path, PathBuf};

use lattice_cloud_client::{
    default_client, BackupMetadataResponse, CloudApiClient, CloudHttpClient, HttpCloudClient,
};
use lattice_core::{Workspace, WorkspaceManifest};
use lattice_storage::atomic_write_file;
use lattice_workspace_crypto::{
    build_workspace_backup_payload, decrypt_blob, is_backup_envelope, open_backup_envelope,
    parse_workspace_backup_payload, seal_backup_envelope, BackupPayload, Dek,
};
use serde::Serialize;

use crate::cloud::resolve_cloud_bearer_cmd;
use crate::path::validate_workspace_relative;
use crate::workspace_crypto::{with_unlocked_session, workspace_crypto_import_dek};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBackupPutResult {
    pub backup_id: String,
    pub cloud_workspace_id: String,
    pub content_hash: String,
    pub ciphertext_bytes: u64,
    pub plaintext_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBackupSkippedEntry {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBackupRestoreResult {
    pub backup_id: String,
    pub restored_count: u64,
    pub skipped: Vec<EncryptedBackupSkippedEntry>,
}

/// Backup metadata for the webview. Omits `object_key` so storage paths stay in Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBackupListEntry {
    pub id: String,
    pub workspace_id: String,
    pub device_id: Option<String>,
    pub size: i64,
    pub content_hash: String,
    pub created_at: i64,
}

impl From<BackupMetadataResponse> for EncryptedBackupListEntry {
    fn from(meta: BackupMetadataResponse) -> Self {
        Self {
            id: meta.id,
            workspace_id: meta.workspace_id,
            device_id: meta.device_id,
            size: meta.size,
            content_hash: meta.content_hash,
            created_at: meta.created_at,
        }
    }
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn api_client() -> CloudApiClient<HttpCloudClient> {
    default_client()
}

/// Encrypt a workspace payload and `PUT` opaque ciphertext to cloud backup storage.
///
/// Requires a signed-in cloud session and an unlocked workspace crypto session for
/// the open workspace's manifest id.
pub fn put_encrypted_workspace_backup(root: &str) -> Result<EncryptedBackupPutResult, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    put_encrypted_workspace_backup_with_client(&api_client(), &bearer, root)
}

pub fn put_encrypted_workspace_backup_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
) -> Result<EncryptedBackupPutResult, String> {
    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let local_id = manifest.id.to_string();
    let workspace_root = workspace.root();

    let plaintext = build_workspace_backup_payload(workspace_root).map_err(map_err)?;
    let wrap_key = fetch_account_wrap_key(client, bearer)?;
    let envelope = with_unlocked_session(&local_id, |session| {
        let ciphertext = session.encrypt_blob(&plaintext).map_err(map_err)?;
        let wrapped_dek = session.wrap_unlocked_dek(&wrap_key).map_err(map_err)?;
        seal_backup_envelope(&wrapped_dek, &ciphertext).map_err(map_err)
    })?;

    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &local_id, manifest.title.as_str())?;
    let metadata = client
        .put_workspace_backup(bearer, &cloud_workspace.id, &envelope, None)
        .map_err(map_err)?;

    Ok(EncryptedBackupPutResult {
        backup_id: metadata.id,
        cloud_workspace_id: metadata.workspace_id,
        content_hash: metadata.content_hash,
        ciphertext_bytes: envelope.len() as u64,
        plaintext_bytes: plaintext.len() as u64,
    })
}

/// List encrypted workspace backups for the open workspace's cloud row.
///
/// HTTP-only: does not unlock the workspace DEK. `object_key` is stripped before
/// returning to the webview.
pub fn list_encrypted_workspace_backups(
    root: &str,
) -> Result<Vec<EncryptedBackupListEntry>, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    list_encrypted_workspace_backups_with_client(&api_client(), &bearer, root)
}

pub fn list_encrypted_workspace_backups_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
) -> Result<Vec<EncryptedBackupListEntry>, String> {
    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let local_id = manifest.id.to_string();
    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &local_id, manifest.title.as_str())?;
    let list = client
        .list_workspace_backups(bearer, &cloud_workspace.id)
        .map_err(map_err)?;
    Ok(list
        .into_iter()
        .map(EncryptedBackupListEntry::from)
        .collect())
}

/// Download, decrypt, and restore an encrypted workspace backup into `target_root`.
///
/// Conflict-safe: existing destination files with different bytes are skipped (not
/// overwritten). When `backup_id` is omitted, uses the latest backup from the list
/// (`created_at` DESC).
pub fn restore_encrypted_workspace_backup(
    root: &str,
    target_root: &str,
    backup_id: Option<&str>,
) -> Result<EncryptedBackupRestoreResult, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    restore_encrypted_workspace_backup_with_client(
        &api_client(),
        &bearer,
        root,
        target_root,
        backup_id,
    )
}

pub fn restore_encrypted_workspace_backup_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    target_root: &str,
    backup_id: Option<&str>,
) -> Result<EncryptedBackupRestoreResult, String> {
    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let local_id = manifest.id.to_string();

    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &local_id, manifest.title.as_str())?;

    let resolved_backup_id = match backup_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => id.to_string(),
        None => {
            let list = client
                .list_workspace_backups(bearer, &cloud_workspace.id)
                .map_err(map_err)?;
            list.into_iter()
                .next()
                .map(|meta| meta.id)
                .ok_or_else(|| "no workspace backups found in cloud".to_string())?
        }
    };

    let ciphertext = client
        .get_workspace_backup(bearer, &cloud_workspace.id, &resolved_backup_id)
        .map_err(map_err)?;
    let payload = decrypt_restore_payload(client, bearer, &local_id, &ciphertext)?;

    let restore = restore_payload_into_target(Path::new(target_root), &payload)?;
    Ok(EncryptedBackupRestoreResult {
        backup_id: resolved_backup_id,
        restored_count: restore.0,
        skipped: restore.1,
    })
}

fn restore_payload_into_target(
    target_root: &Path,
    payload: &BackupPayload,
) -> Result<(u64, Vec<EncryptedBackupSkippedEntry>), String> {
    fs::create_dir_all(target_root)
        .map_err(|err| format!("create restore target {}: {err}", target_root.display()))?;

    let mut restored_count = 0u64;
    let mut skipped = Vec::new();

    apply_restore_file(
        target_root,
        "lattice.yaml",
        &payload.manifest,
        &mut restored_count,
        &mut skipped,
    )?;

    for (rel, bytes) in &payload.files {
        validate_workspace_relative(rel)?;
        apply_restore_file(target_root, rel, bytes, &mut restored_count, &mut skipped)?;
    }

    Ok((restored_count, skipped))
}

fn apply_restore_file(
    target_root: &Path,
    rel: &str,
    bytes: &[u8],
    restored_count: &mut u64,
    skipped: &mut Vec<EncryptedBackupSkippedEntry>,
) -> Result<(), String> {
    let dest = join_restore_path(target_root, rel)?;
    if dest.exists() {
        let existing =
            fs::read(&dest).map_err(|err| format!("read existing {}: {err}", dest.display()))?;
        if existing != bytes {
            skipped.push(EncryptedBackupSkippedEntry {
                path: rel.to_string(),
                reason: "destination exists with different content".into(),
            });
            return Ok(());
        }
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent {}: {err}", parent.display()))?;
    }
    atomic_write_file(&dest, bytes).map_err(map_err)?;
    *restored_count += 1;
    Ok(())
}

fn join_restore_path(target_root: &Path, rel: &str) -> Result<PathBuf, String> {
    let relative = validate_workspace_relative(rel)?;
    let candidate = target_root.join(&relative);
    // Defense in depth: reject any join that leaves the target root prefix.
    if !candidate.starts_with(target_root) {
        return Err(format!("{rel:?} escapes the restore target root"));
    }
    Ok(candidate)
}

/// Resolve or create the cloud workspace row bound to a local manifest id.
pub fn ensure_cloud_workspace<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    local_workspace_id: &str,
    title: &str,
) -> Result<lattice_cloud_client::CloudWorkspaceRecord, String> {
    let workspaces = client.list_workspaces(bearer).map_err(map_err)?;
    if let Some(existing) = workspaces.into_iter().find(|ws| {
        ws.local_workspace_id
            .as_deref()
            .is_some_and(|id| id == local_workspace_id)
    }) {
        return Ok(existing);
    }
    client
        .create_workspace(bearer, title, Some(local_workspace_id))
        .map_err(map_err)
}

fn fetch_account_wrap_key<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
) -> Result<Dek, String> {
    let response = client.get_backup_wrap_key(bearer).map_err(map_err)?;
    let bytes = hex::decode(response.wrap_key.trim())
        .map_err(|err| format!("invalid backup wrap key encoding: {err}"))?;
    Dek::try_from_slice(&bytes).map_err(map_err)
}

fn decrypt_restore_payload<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    local_id: &str,
    body: &[u8],
) -> Result<BackupPayload, String> {
    if is_backup_envelope(body) {
        let wrap_key = fetch_account_wrap_key(client, bearer)?;
        let (dek, inner) = open_backup_envelope(&wrap_key, body).map_err(|err| {
            format!("failed to unwrap backup DEK (will not provision a new key): {err}")
        })?;
        let plaintext = decrypt_blob(&dek, &inner).map_err(map_err)?;
        let payload = parse_workspace_backup_payload(&plaintext).map_err(map_err)?;
        let payload_workspace_id = workspace_id_from_manifest(&payload.manifest)?;
        workspace_crypto_import_dek(&payload_workspace_id, dek)?;
        Ok(payload)
    } else {
        let plaintext = with_unlocked_session(local_id, |session| {
            session.decrypt_blob(body).map_err(map_err)
        })?;
        parse_workspace_backup_payload(&plaintext).map_err(map_err)
    }
}

fn workspace_id_from_manifest(bytes: &[u8]) -> Result<String, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|err| format!("backup manifest is not UTF-8: {err}"))?;
    let manifest = WorkspaceManifest::parse(Path::new("lattice.yaml"), text).map_err(map_err)?;
    Ok(manifest.id.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use lattice_cloud_client::{
        CloudApiClient, CloudError, CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse,
    };
    use lattice_core::Workspace;
    use lattice_workspace_crypto::{
        build_workspace_backup_payload, decrypt_blob, is_backup_envelope, open_backup_envelope,
        parse_workspace_backup_payload, Dek, MemoryKeystore, WorkspaceCryptoSession,
        ENVELOPE_MAGIC,
    };
    use latticefs_core::ContentHash;

    use super::*;
    use crate::workspace_crypto::{
        with_unlocked_session, workspace_crypto_destroy, workspace_crypto_lock,
        workspace_crypto_unlock,
    };

    const TEST_WRAP_KEY_HEX: &str =
        "4242424242424242424242424242424242424242424242424242424242424242";

    #[derive(Default, Clone)]
    struct FakeHttp {
        responses: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
        bytes_responses: Arc<Mutex<HashMap<String, CloudHttpBytesResponse>>>,
        captured_put: Arc<Mutex<Option<Vec<u8>>>>,
    }

    impl FakeHttp {
        fn insert(&self, method: &str, path: &str, response: CloudHttpResponse) {
            self.responses
                .lock()
                .unwrap()
                .insert(format!("{method} {path}"), response);
        }

        fn insert_bytes(&self, method: &str, path: &str, response: CloudHttpBytesResponse) {
            self.bytes_responses
                .lock()
                .unwrap()
                .insert(format!("{method} {path}"), response);
        }
    }

    impl CloudHttpClient for FakeHttp {
        fn request(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            _body: Option<&serde_json::Value>,
            bearer: Option<&str>,
        ) -> Result<CloudHttpResponse, CloudError> {
            let key = format!("{method} {path}");
            if bearer != Some("tok") {
                return Ok(CloudHttpResponse {
                    status: 401,
                    body: r#"{"error":"invalid session"}"#.into(),
                });
            }
            self.responses
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake response for {key}")))
        }

        fn request_bytes(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            body: Option<&[u8]>,
            bearer: Option<&str>,
            _headers: &[(&str, &str)],
        ) -> Result<CloudHttpBytesResponse, CloudError> {
            if bearer != Some("tok") {
                return Ok(CloudHttpBytesResponse {
                    status: 401,
                    body: br#"{"error":"invalid session"}"#.to_vec(),
                    content_hash: None,
                });
            }
            if method == "PUT" && path.ends_with("/backups") {
                let body =
                    body.ok_or_else(|| CloudError::Http("backup PUT missing body".into()))?;
                *self.captured_put.lock().unwrap() = Some(body.to_vec());
                let hash_hex = ContentHash::from_bytes(body)
                    .map_err(|err| CloudError::Http(err.to_string()))?
                    .as_str()
                    .strip_prefix("sha256:")
                    .unwrap_or_default()
                    .to_string();
                let workspace_id = path
                    .trim_start_matches("/v1/workspaces/")
                    .trim_end_matches("/backups");
                return Ok(CloudHttpBytesResponse {
                    status: 201,
                    body: format!(
                        r#"{{"id":"bk-1","workspace_id":"{workspace_id}","device_id":null,"object_key":"backups/{workspace_id}/bk-1","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                        body.len()
                    )
                    .into_bytes(),
                    content_hash: None,
                });
            }
            let key = format!("{method} {path}");
            self.bytes_responses
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake bytes response for {key}")))
        }
    }

    fn seed_workspace_cloud(http: &FakeHttp, local_id: &str) {
        http.insert(
            "GET",
            "/v1/workspaces",
            CloudHttpResponse {
                status: 200,
                body: format!(
                    r#"[{{"id":"cloud-ws-1","owner_user_id":"u1","name":"EncBackup","local_workspace_id":"{local_id}","created_at":1}}]"#
                ),
            },
        );
    }

    fn seed_wrap_key(http: &FakeHttp) {
        http.insert(
            "GET",
            "/v1/me/backup-wrap-key",
            CloudHttpResponse {
                status: 200,
                body: format!(r#"{{"wrap_key":"{TEST_WRAP_KEY_HEX}"}}"#),
            },
        );
    }

    fn test_wrap_key() -> Dek {
        Dek::try_from_slice(&hex::decode(TEST_WRAP_KEY_HEX).unwrap()).unwrap()
    }

    #[test]
    fn payload_encrypt_decrypt_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "EncBackup").unwrap();
        std::fs::write(dir.path().join("Notes.md"), b"secret notes").unwrap();

        let store = MemoryKeystore::new();
        let mut session = WorkspaceCryptoSession::new(store);
        session.provision("ws-local").unwrap();
        let plaintext = build_workspace_backup_payload(dir.path()).unwrap();
        let ciphertext = session.encrypt_blob(&plaintext).unwrap();
        let decrypted = session.decrypt_blob(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
        assert_ne!(ciphertext, plaintext);
    }

    #[test]
    fn encrypt_put_with_mock_cloud() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "EncBackup").unwrap();
        std::fs::write(dir.path().join("Notes.md"), b"secret notes").unwrap();
        let local_id = Workspace::open(dir.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let _ = workspace_crypto_lock();
        workspace_crypto_unlock(local_id.clone()).unwrap();

        let http = FakeHttp {
            captured_put: Arc::new(Mutex::new(None)),
            ..Default::default()
        };
        http.insert(
            "GET",
            "/v1/workspaces",
            CloudHttpResponse {
                status: 200,
                body: "[]".into(),
            },
        );
        http.insert(
            "POST",
            "/v1/workspaces",
            CloudHttpResponse {
                status: 201,
                body: format!(
                    r#"{{"id":"cloud-ws-1","owner_user_id":"u1","name":"EncBackup","local_workspace_id":"{local_id}","created_at":1}}"#
                ),
            },
        );
        seed_wrap_key(&http);

        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let root = dir.path().to_string_lossy().into_owned();
        let plaintext = build_workspace_backup_payload(dir.path()).unwrap();
        let result = put_encrypted_workspace_backup_with_client(&client, "tok", &root).unwrap();
        assert_eq!(result.backup_id, "bk-1");
        assert_eq!(result.plaintext_bytes, plaintext.len() as u64);

        let captured = http.captured_put.lock().unwrap().clone().expect("PUT body");
        assert!(captured.starts_with(ENVELOPE_MAGIC));
        assert!(is_backup_envelope(&captured));

        let (dek, inner) = open_backup_envelope(&test_wrap_key(), &captured).unwrap();
        let round_trip = decrypt_blob(&dek, &inner).unwrap();
        assert_eq!(round_trip, plaintext);
        let session_round_trip = with_unlocked_session(&local_id, |session| {
            session.decrypt_blob(&inner).map_err(|err| err.to_string())
        })
        .unwrap();
        assert_eq!(session_round_trip, plaintext);
    }

    #[test]
    fn restore_from_mock_cloud_conflict_safe() {
        let src = tempfile::tempdir().unwrap();
        Workspace::init(src.path(), "EncBackup").unwrap();
        std::fs::write(src.path().join("Notes.md"), b"secret notes").unwrap();
        std::fs::create_dir_all(src.path().join("nested")).unwrap();
        std::fs::write(src.path().join("nested/a.txt"), b"nested-a").unwrap();
        let local_id = Workspace::open(src.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let _ = workspace_crypto_lock();
        workspace_crypto_unlock(local_id.clone()).unwrap();

        let plaintext = build_workspace_backup_payload(src.path()).unwrap();
        let ciphertext = with_unlocked_session(&local_id, |session| {
            session
                .encrypt_blob(&plaintext)
                .map_err(|err| err.to_string())
        })
        .unwrap();
        let hash_hex = ContentHash::from_bytes(&ciphertext)
            .unwrap()
            .as_str()
            .strip_prefix("sha256:")
            .unwrap()
            .to_string();

        let http = FakeHttp::default();
        seed_workspace_cloud(&http, &local_id);
        http.insert(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups",
            CloudHttpResponse {
                status: 200,
                body: format!(
                    r#"[{{"id":"bk-latest","workspace_id":"cloud-ws-1","device_id":null,"object_key":"backups/cloud-ws-1/bk-latest","size":{},"content_hash":"{hash_hex}","created_at":99}},{{"id":"bk-old","workspace_id":"cloud-ws-1","device_id":null,"object_key":"backups/cloud-ws-1/bk-old","size":1,"content_hash":"00","created_at":1}}]"#,
                    ciphertext.len()
                ),
            },
        );
        http.insert_bytes(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups/bk-latest",
            CloudHttpBytesResponse {
                status: 200,
                body: ciphertext.clone(),
                content_hash: Some(hash_hex),
            },
        );

        let target = tempfile::tempdir().unwrap();
        // Conflict: Notes.md differs → skip. nested/a.txt missing → restore.
        // lattice.yaml missing → restore.
        std::fs::write(target.path().join("Notes.md"), b"local different").unwrap();

        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let root = src.path().to_string_lossy().into_owned();
        let target_root = target.path().to_string_lossy().into_owned();
        let result = restore_encrypted_workspace_backup_with_client(
            &client,
            "tok",
            &root,
            &target_root,
            None,
        )
        .unwrap();

        assert_eq!(result.backup_id, "bk-latest");
        assert!(result.restored_count >= 2);
        assert!(result
            .skipped
            .iter()
            .any(|s| s.path == "Notes.md" && s.reason.contains("different content")));
        assert_eq!(
            std::fs::read(target.path().join("Notes.md")).unwrap(),
            b"local different"
        );
        assert_eq!(
            std::fs::read(target.path().join("nested/a.txt")).unwrap(),
            b"nested-a"
        );
        assert!(target.path().join("lattice.yaml").is_file());

        let restored_manifest = std::fs::read(target.path().join("lattice.yaml")).unwrap();
        let parsed = parse_workspace_backup_payload(&plaintext).unwrap();
        assert_eq!(restored_manifest, parsed.manifest);
    }

    #[test]
    fn restore_specific_backup_id() {
        let src = tempfile::tempdir().unwrap();
        Workspace::init(src.path(), "EncBackup").unwrap();
        std::fs::write(src.path().join("Only.md"), b"only").unwrap();
        let local_id = Workspace::open(src.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let _ = workspace_crypto_lock();
        workspace_crypto_unlock(local_id.clone()).unwrap();

        let plaintext = build_workspace_backup_payload(src.path()).unwrap();
        let ciphertext = with_unlocked_session(&local_id, |session| {
            session
                .encrypt_blob(&plaintext)
                .map_err(|err| err.to_string())
        })
        .unwrap();
        let hash_hex = ContentHash::from_bytes(&ciphertext)
            .unwrap()
            .as_str()
            .strip_prefix("sha256:")
            .unwrap()
            .to_string();

        let http = FakeHttp::default();
        seed_workspace_cloud(&http, &local_id);
        http.insert_bytes(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups/bk-specific",
            CloudHttpBytesResponse {
                status: 200,
                body: ciphertext,
                content_hash: Some(hash_hex),
            },
        );

        let target = tempfile::tempdir().unwrap();
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let result = restore_encrypted_workspace_backup_with_client(
            &client,
            "tok",
            &src.path().to_string_lossy(),
            &target.path().to_string_lossy(),
            Some("bk-specific"),
        )
        .unwrap();
        assert_eq!(result.backup_id, "bk-specific");
        assert_eq!(
            std::fs::read(target.path().join("Only.md")).unwrap(),
            b"only"
        );
        assert!(result.skipped.is_empty());
    }

    #[test]
    fn list_encrypted_backups_round_trip_strips_object_key() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "EncBackup").unwrap();
        let local_id = Workspace::open(dir.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let http = FakeHttp::default();
        seed_workspace_cloud(&http, &local_id);
        http.insert(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups",
            CloudHttpResponse {
                status: 200,
                body: r#"[{"id":"bk-latest","workspace_id":"cloud-ws-1","device_id":"dev-1","object_key":"backups/cloud-ws-1/bk-latest","size":42,"content_hash":"abc123","created_at":99},{"id":"bk-old","workspace_id":"cloud-ws-1","device_id":null,"object_key":"backups/cloud-ws-1/bk-old","size":1,"content_hash":"00","created_at":1}]"#.into(),
            },
        );

        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let list = list_encrypted_workspace_backups_with_client(
            &client,
            "tok",
            &dir.path().to_string_lossy(),
        )
        .unwrap();

        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0],
            EncryptedBackupListEntry {
                id: "bk-latest".into(),
                workspace_id: "cloud-ws-1".into(),
                device_id: Some("dev-1".into()),
                size: 42,
                content_hash: "abc123".into(),
                created_at: 99,
            }
        );
        assert_eq!(list[1].id, "bk-old");
        assert_eq!(list[1].device_id, None);

        let json = serde_json::to_value(&list[0]).unwrap();
        assert!(json.get("objectKey").is_none());
        assert!(json.get("object_key").is_none());
        assert_eq!(json["id"], "bk-latest");
        assert_eq!(json["workspaceId"], "cloud-ws-1");
        assert_eq!(json["deviceId"], "dev-1");
        assert_eq!(json["contentHash"], "abc123");
        assert_eq!(json["createdAt"], 99);
    }

    #[test]
    fn restore_envelope_with_empty_keystore() {
        let src = tempfile::tempdir().unwrap();
        Workspace::init(src.path(), "EncBackup").unwrap();
        std::fs::write(src.path().join("Notes.md"), b"secret notes").unwrap();
        let local_id = Workspace::open(src.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let _ = workspace_crypto_lock();
        workspace_crypto_unlock(local_id.clone()).unwrap();

        let http = FakeHttp {
            captured_put: Arc::new(Mutex::new(None)),
            ..Default::default()
        };
        seed_workspace_cloud(&http, &local_id);
        seed_wrap_key(&http);

        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let root = src.path().to_string_lossy().into_owned();
        let put = put_encrypted_workspace_backup_with_client(&client, "tok", &root).unwrap();
        let envelope = http.captured_put.lock().unwrap().clone().expect("PUT body");
        assert!(is_backup_envelope(&envelope));

        workspace_crypto_destroy(&local_id).unwrap();
        let _ = workspace_crypto_lock();
        assert!(!crate::workspace_crypto::workspace_crypto_status().unlocked);

        let hash_hex = ContentHash::from_bytes(&envelope)
            .unwrap()
            .as_str()
            .strip_prefix("sha256:")
            .unwrap()
            .to_string();
        http.insert_bytes(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups/bk-1",
            CloudHttpBytesResponse {
                status: 200,
                body: envelope,
                content_hash: Some(hash_hex),
            },
        );

        let target = tempfile::tempdir().unwrap();
        let result = restore_encrypted_workspace_backup_with_client(
            &client,
            "tok",
            &root,
            &target.path().to_string_lossy(),
            Some("bk-1"),
        )
        .unwrap();
        assert_eq!(result.backup_id, put.backup_id);
        assert_eq!(
            std::fs::read(target.path().join("Notes.md")).unwrap(),
            b"secret notes"
        );
        assert!(crate::workspace_crypto::workspace_crypto_status().unlocked);
        assert_eq!(
            crate::workspace_crypto::workspace_crypto_status()
                .workspace_id
                .as_deref(),
            Some(local_id.as_str())
        );
    }

    #[test]
    fn restore_envelope_unwrap_failure_does_not_provision() {
        let src = tempfile::tempdir().unwrap();
        Workspace::init(src.path(), "EncBackup").unwrap();
        std::fs::write(src.path().join("Notes.md"), b"secret notes").unwrap();
        let local_id = Workspace::open(src.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let _ = workspace_crypto_lock();
        workspace_crypto_unlock(local_id.clone()).unwrap();

        let http = FakeHttp {
            captured_put: Arc::new(Mutex::new(None)),
            ..Default::default()
        };
        seed_workspace_cloud(&http, &local_id);
        seed_wrap_key(&http);
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let root = src.path().to_string_lossy().into_owned();
        put_encrypted_workspace_backup_with_client(&client, "tok", &root).unwrap();
        let envelope = http.captured_put.lock().unwrap().clone().expect("PUT body");

        workspace_crypto_destroy(&local_id).unwrap();
        let _ = workspace_crypto_lock();

        http.insert(
            "GET",
            "/v1/me/backup-wrap-key",
            CloudHttpResponse {
                status: 200,
                body: r#"{"wrap_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#.into(),
            },
        );
        http.insert_bytes(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups/bk-1",
            CloudHttpBytesResponse {
                status: 200,
                body: envelope,
                content_hash: None,
            },
        );

        let target = tempfile::tempdir().unwrap();
        let err = restore_encrypted_workspace_backup_with_client(
            &client,
            "tok",
            &root,
            &target.path().to_string_lossy(),
            Some("bk-1"),
        )
        .unwrap_err();
        assert!(
            err.contains("failed to unwrap backup DEK"),
            "unexpected error: {err}"
        );
        assert!(!crate::workspace_crypto::workspace_crypto_status().unlocked);
        assert!(!target.path().join("Notes.md").exists());
    }

    #[test]
    fn list_encrypted_backups_empty() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "EncBackup").unwrap();
        let local_id = Workspace::open(dir.path())
            .unwrap()
            .manifest()
            .id
            .to_string();

        let http = FakeHttp::default();
        seed_workspace_cloud(&http, &local_id);
        http.insert(
            "GET",
            "/v1/workspaces/cloud-ws-1/backups",
            CloudHttpResponse {
                status: 200,
                body: "[]".into(),
            },
        );

        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let list = list_encrypted_workspace_backups_with_client(
            &client,
            "tok",
            &dir.path().to_string_lossy(),
        )
        .unwrap();
        assert!(list.is_empty());
    }
}
