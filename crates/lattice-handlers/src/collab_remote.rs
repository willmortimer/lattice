//! Cloud-backed Yrs remote snapshot push/pull (S8).
//!
//! Uses existing workspace-scoped blob PUT/GET with a sidecar ResourceId from
//! [`lattice_collab::collab_snapshot_resource_id`]. Local collab journal remains
//! authoritative; remote is an opaque peer-exchange snapshot.

use std::path::Path;

use lattice_cloud_client::{default_client, CloudApiClient, CloudError, CloudHttpClient, HttpCloudClient};
use lattice_collab::{
    collab_snapshot_resource_id, decode_remote_snapshot, encode_remote_snapshot, parse_doc_resource_id,
};
use lattice_core::Workspace;
use latticefs_core::ContentHash;
use serde::Serialize;

use crate::cloud::resolve_cloud_bearer_cmd;
use crate::workspace_backup::ensure_cloud_workspace;

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn api_client() -> CloudApiClient<HttpCloudClient> {
    default_client()
}

fn hash_hex(hash: &ContentHash) -> String {
    hash.as_str()
        .strip_prefix("sha256:")
        .unwrap_or(hash.as_str())
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabRemotePushResult {
    pub page_id: String,
    pub sidecar_id: String,
    pub cloud_workspace_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabRemotePullResult {
    pub page_id: String,
    pub sidecar_id: String,
    pub cloud_workspace_id: String,
    pub content_hash: String,
    pub update: Vec<u8>,
}

/// Push a full Yrs update for `page_resource_id` to the cloud sidecar blob.
pub fn push_collab_remote_snapshot(
    root: &str,
    page_resource_id: &str,
    yrs_update: &[u8],
    if_match: Option<&str>,
) -> Result<CollabRemotePushResult, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    push_collab_remote_snapshot_with_client(
        &api_client(),
        &bearer,
        root,
        page_resource_id,
        yrs_update,
        if_match,
    )
}

pub fn push_collab_remote_snapshot_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    page_resource_id: &str,
    yrs_update: &[u8],
    if_match: Option<&str>,
) -> Result<CollabRemotePushResult, String> {
    let page_id = parse_doc_resource_id(page_resource_id).map_err(map_err)?;
    let sidecar_id = collab_snapshot_resource_id(page_id);
    let payload = encode_remote_snapshot(page_id, yrs_update);

    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &manifest.id.to_string(), manifest.title.as_str())?;

    let hash = client
        .put_workspace_blob(
            bearer,
            Some(&cloud_workspace.id),
            sidecar_id,
            &payload,
            if_match,
        )
        .map_err(map_err)?;

    Ok(CollabRemotePushResult {
        page_id: page_id.to_string(),
        sidecar_id: sidecar_id.to_string(),
        cloud_workspace_id: cloud_workspace.id,
        content_hash: hash_hex(&hash),
    })
}

/// Pull the remote Yrs snapshot for `page_resource_id`, if present.
pub fn pull_collab_remote_snapshot(
    root: &str,
    page_resource_id: &str,
) -> Result<Option<CollabRemotePullResult>, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    pull_collab_remote_snapshot_with_client(&api_client(), &bearer, root, page_resource_id)
}

pub fn pull_collab_remote_snapshot_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    page_resource_id: &str,
) -> Result<Option<CollabRemotePullResult>, String> {
    let page_id = parse_doc_resource_id(page_resource_id).map_err(map_err)?;
    let sidecar_id = collab_snapshot_resource_id(page_id);

    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let cloud_workspace =
        ensure_cloud_workspace(client, bearer, &manifest.id.to_string(), manifest.title.as_str())?;

    let bytes = match client.get_blob(bearer, sidecar_id) {
        Ok(bytes) => bytes,
        Err(CloudError::Api { status: 404, .. }) => return Ok(None),
        Err(err) => return Err(map_err(err)),
    };
    let update = decode_remote_snapshot(page_id, &bytes).map_err(map_err)?;
    let hash = ContentHash::from_bytes(&bytes).map_err(map_err)?;

    Ok(Some(CollabRemotePullResult {
        page_id: page_id.to_string(),
        sidecar_id: sidecar_id.to_string(),
        cloud_workspace_id: cloud_workspace.id,
        content_hash: hash_hex(&hash),
        update,
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use lattice_cloud_client::{
        CloudApiClient, CloudError, CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse,
        WORKSPACE_ID_HEADER,
    };
    use lattice_collab::{collab_snapshot_resource_id, encode_remote_snapshot};
    use lattice_core::Workspace;
    use latticefs_core::{ContentHash, ResourceId};
    use yrs::{Doc, ReadTxn, Text, Transact};

    use super::*;

    #[derive(Default, Clone)]
    struct FakeHttp {
        json: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
        blobs: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    }

    impl CloudHttpClient for FakeHttp {
        fn request(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            body: Option<&serde_json::Value>,
            bearer: Option<&str>,
        ) -> Result<CloudHttpResponse, CloudError> {
            if bearer != Some("tok") {
                return Ok(CloudHttpResponse {
                    status: 401,
                    body: r#"{"error":"invalid session"}"#.into(),
                });
            }
            if method == "POST" && path == "/v1/workspaces" {
                let local = body
                    .and_then(|v| v.get("local_workspace_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("local");
                return Ok(CloudHttpResponse {
                    status: 201,
                    body: format!(
                        r#"{{"id":"cloud-ws-1","owner_user_id":"u1","name":"Demo","local_workspace_id":"{local}","created_at":1}}"#
                    ),
                });
            }
            if method == "GET" && path == "/v1/workspaces" {
                return Ok(CloudHttpResponse {
                    status: 200,
                    body: "[]".into(),
                });
            }
            self.json
                .lock()
                .unwrap()
                .get(&format!("{method} {path}"))
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake json for {method} {path}")))
        }

        fn request_bytes(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            body: Option<&[u8]>,
            bearer: Option<&str>,
            headers: &[(&str, &str)],
        ) -> Result<CloudHttpBytesResponse, CloudError> {
            if bearer != Some("tok") {
                return Ok(CloudHttpBytesResponse {
                    status: 401,
                    body: br#"{"error":"invalid session"}"#.to_vec(),
                    content_hash: None,
                });
            }
            let resource_id = path.trim_start_matches("/v1/blobs/");
            if method == "PUT" {
                assert!(headers
                    .iter()
                    .any(|(n, v)| *n == WORKSPACE_ID_HEADER && *v == "cloud-ws-1"));
                let data = body.expect("put body");
                let hash = ContentHash::from_bytes(data).unwrap();
                let hash_hex = hash.as_str().strip_prefix("sha256:").unwrap();
                self.blobs
                    .lock()
                    .unwrap()
                    .insert(resource_id.to_string(), data.to_vec());
                return Ok(CloudHttpBytesResponse {
                    status: 201,
                    body: format!(
                        r#"{{"resource_id":"{resource_id}","object_key":"blobs/u1/sha256/{hash_hex}","size":{},"content_hash":"{hash_hex}","created_at":1}}"#,
                        data.len()
                    )
                    .into_bytes(),
                    content_hash: None,
                });
            }
            if method == "GET" {
                match self.blobs.lock().unwrap().get(resource_id) {
                    Some(bytes) => {
                        let hash = ContentHash::from_bytes(bytes).unwrap();
                        let hash_hex = hash.as_str().strip_prefix("sha256:").unwrap().to_string();
                        return Ok(CloudHttpBytesResponse {
                            status: 200,
                            body: bytes.clone(),
                            content_hash: Some(hash_hex),
                        });
                    }
                    None => {
                        return Ok(CloudHttpBytesResponse {
                            status: 404,
                            body: br#"{"error":"not found"}"#.to_vec(),
                            content_hash: None,
                        });
                    }
                }
            }
            Err(CloudError::Http(format!("unexpected {method} {path}")))
        }
    }

    fn make_text_update(text: &str) -> Vec<u8> {
        let doc = Doc::new();
        let shared = doc.get_or_insert_text("content");
        {
            let mut txn = doc.transact_mut();
            shared.push(&mut txn, text);
        }
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&yrs::StateVector::default())
    }

    #[test]
    fn push_then_pull_roundtrip_via_cloud_blob() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let update = make_text_update("cloud peer");

        let http = FakeHttp::default();
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");

        let pushed = push_collab_remote_snapshot_with_client(
            &client,
            "tok",
            dir.path().to_str().unwrap(),
            &page.to_string(),
            &update,
            None,
        )
        .unwrap();
        assert_eq!(pushed.page_id, page.to_string());
        assert_eq!(
            pushed.sidecar_id,
            collab_snapshot_resource_id(page).to_string()
        );

        let pulled = pull_collab_remote_snapshot_with_client(
            &client,
            "tok",
            dir.path().to_str().unwrap(),
            &page.to_string(),
        )
        .unwrap()
        .expect("snapshot present");
        assert_eq!(pulled.update, update);
        assert_eq!(pulled.content_hash, pushed.content_hash);

        // Sidecar stores wrapped payload, not raw Yrs bytes.
        let raw = http
            .blobs
            .lock()
            .unwrap()
            .get(&pushed.sidecar_id)
            .cloned()
            .unwrap();
        assert_eq!(raw, encode_remote_snapshot(page, &update));
    }

    #[test]
    fn pull_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let http = FakeHttp::default();
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let pulled = pull_collab_remote_snapshot_with_client(
            &client,
            "tok",
            dir.path().to_str().unwrap(),
            &page.to_string(),
        )
        .unwrap();
        assert!(pulled.is_none());
    }
}
