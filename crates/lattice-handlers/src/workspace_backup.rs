//! Encrypted workspace backup PUT (opaque ciphertext to lattice-server).

use std::path::Path;

use lattice_cloud_client::{default_client, CloudApiClient, CloudHttpClient, HttpCloudClient};
use lattice_core::Workspace;
use lattice_workspace_crypto::build_workspace_backup_payload;
use serde::Serialize;

use crate::cloud::resolve_cloud_bearer_cmd;
use crate::workspace_crypto::with_unlocked_session;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBackupPutResult {
    pub backup_id: String,
    pub cloud_workspace_id: String,
    pub content_hash: String,
    pub ciphertext_bytes: u64,
    pub plaintext_bytes: u64,
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
    let ciphertext = with_unlocked_session(&local_id, |session| {
        session.encrypt_blob(&plaintext).map_err(map_err)
    })?;

    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &local_id, manifest.title.as_str())?;
    let metadata = client
        .put_workspace_backup(bearer, &cloud_workspace.id, &ciphertext, None)
        .map_err(map_err)?;

    Ok(EncryptedBackupPutResult {
        backup_id: metadata.id,
        cloud_workspace_id: metadata.workspace_id,
        content_hash: metadata.content_hash,
        ciphertext_bytes: ciphertext.len() as u64,
        plaintext_bytes: plaintext.len() as u64,
    })
}

fn ensure_cloud_workspace<C: CloudHttpClient>(
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use lattice_cloud_client::{
        CloudApiClient, CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse, CloudError,
    };
    use lattice_core::Workspace;
    use lattice_workspace_crypto::{MemoryKeystore, WorkspaceCryptoSession};

    use super::*;
    use crate::workspace_crypto::{with_unlocked_session, workspace_crypto_lock, workspace_crypto_unlock};

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
            if method == "PUT" && path.contains("/backups") {
                let body = body.ok_or_else(|| {
                    CloudError::Http("backup PUT missing body".into())
                })?;
                *self.captured_put.lock().unwrap() = Some(body.to_vec());
                let hash_hex = latticefs_core::ContentHash::from_bytes(body)
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
        let local_id = Workspace::open(dir.path()).unwrap().manifest().id.to_string();

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

        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let root = dir.path().to_string_lossy().into_owned();
        let plaintext = build_workspace_backup_payload(dir.path()).unwrap();
        let result =
            put_encrypted_workspace_backup_with_client(&client, "tok", &root).unwrap();
        assert_eq!(result.backup_id, "bk-1");
        assert_eq!(result.plaintext_bytes, plaintext.len() as u64);

        let captured = http.captured_put.lock().unwrap().clone().expect("PUT body");
        assert!(captured.len() > lattice_workspace_crypto::NONCE_LEN);

        let round_trip = with_unlocked_session(&local_id, |session| {
            session.decrypt_blob(&captured).map_err(|err| err.to_string())
        })
        .unwrap();
        assert_eq!(round_trip, plaintext);
    }
}
