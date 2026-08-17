//! Cloud-backed Yrs remote snapshot and append-log push/pull (S8).
//!
//! Uses existing workspace-scoped blob PUT/GET with sidecar ResourceIds from
//! [`lattice_collab::collab_snapshot_resource_id`] / [`lattice_collab::collab_log_resource_id`].
//! Local collab journal remains authoritative; remote is opaque peer-exchange.

use std::path::Path;

use lattice_cloud_client::{
    default_client, CloudApiClient, CloudError, CloudHttpClient, HttpCloudClient,
};
use lattice_collab::{
    append_update, collab_log_resource_id, collab_snapshot_resource_id, decode_remote_log,
    decode_remote_snapshot, encode_remote_log, encode_remote_snapshot, parse_doc_resource_id,
    REMOTE_LOG_UNKNOWN_BASE_HASH,
};
use lattice_core::Workspace;
use latticefs_core::ContentHash;
use serde::Serialize;

use crate::cloud::resolve_cloud_bearer_cmd;
use crate::workspace_backup::ensure_cloud_workspace;

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn map_collab_remote_err(err: lattice_collab::Error) -> String {
    match err {
        lattice_collab::Error::LogNeedsCompact {
            update_count,
            byte_count,
        } => format!("log_needs_compact: {update_count} updates, {byte_count} bytes"),
        other => other.to_string(),
    }
}

/// Parse LYRL `base_hash`: 32 raw SHA-256 bytes, 64 ASCII hex characters, or
/// omit/empty → [`REMOTE_LOG_UNKNOWN_BASE_HASH`].
fn parse_base_hash(raw: Option<&[u8]>) -> Result<[u8; 32], String> {
    match raw {
        None | Some([]) => Ok(REMOTE_LOG_UNKNOWN_BASE_HASH),
        Some(bytes) if bytes.len() == 32 => {
            let mut out = [0u8; 32];
            out.copy_from_slice(bytes);
            Ok(out)
        }
        Some(bytes) if bytes.len() == 64 => {
            let hex_str = std::str::from_utf8(bytes).map_err(map_err)?;
            let decoded = hex::decode(hex_str.trim()).map_err(map_err)?;
            <[u8; 32]>::try_from(decoded)
                .map_err(|_| "base_hash hex must decode to 32 bytes".to_string())
        }
        Some(bytes) => Err(format!(
            "base_hash must be 32 raw SHA-256 bytes or 64 hex characters, got {} bytes",
            bytes.len()
        )),
    }
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
    let cloud_workspace = ensure_cloud_workspace(
        client,
        bearer,
        &manifest.id.to_string(),
        manifest.title.as_str(),
    )?;

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
    let cloud_workspace = ensure_cloud_workspace(
        client,
        bearer,
        &manifest.id.to_string(),
        manifest.title.as_str(),
    )?;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabRemoteLogPushResult {
    pub page_id: String,
    pub sidecar_id: String,
    pub cloud_workspace_id: String,
    pub content_hash: String,
    pub base_hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabRemoteLogPullResult {
    pub page_id: String,
    pub sidecar_id: String,
    pub cloud_workspace_id: String,
    pub content_hash: String,
    pub base_hash: Vec<u8>,
    pub updates: Vec<Vec<u8>>,
}

/// Append one lib0 v1 update to the cloud LYRL sidecar blob for `page_resource_id`.
///
/// Pulls the existing log (404 → empty), runs [`append_update`], then PUTs with
/// `If-Match` set to the pulled blob hash so concurrent peers do not clobber.
///
/// `base_hash` is 32 raw SHA-256 bytes of the LYRS snapshot this log is based
/// on, 64 ASCII hex characters of that digest, or omit/empty for
/// [`REMOTE_LOG_UNKNOWN_BASE_HASH`]. It is applied only when creating a new log;
/// later appends keep the stored base.
pub fn push_collab_remote_log(
    root: &str,
    page_resource_id: &str,
    yrs_update: &[u8],
    base_hash: Option<&[u8]>,
) -> Result<CollabRemoteLogPushResult, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    push_collab_remote_log_with_client(
        &api_client(),
        &bearer,
        root,
        page_resource_id,
        yrs_update,
        base_hash,
    )
}

pub fn push_collab_remote_log_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    page_resource_id: &str,
    yrs_update: &[u8],
    base_hash: Option<&[u8]>,
) -> Result<CollabRemoteLogPushResult, String> {
    let page_id = parse_doc_resource_id(page_resource_id).map_err(map_err)?;
    let sidecar_id = collab_log_resource_id(page_id);

    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let cloud_workspace = ensure_cloud_workspace(
        client,
        bearer,
        &manifest.id.to_string(),
        manifest.title.as_str(),
    )?;

    let existing = match client.get_blob(bearer, sidecar_id) {
        Ok(bytes) => bytes,
        Err(CloudError::Api { status: 404, .. }) => Vec::new(),
        Err(err) => return Err(map_err(err)),
    };
    let if_match = if existing.is_empty() {
        None
    } else {
        let hash = ContentHash::from_bytes(&existing).map_err(map_err)?;
        Some(hash_hex(&hash))
    };

    let appended = append_update(page_id, &existing, yrs_update).map_err(map_collab_remote_err)?;
    let payload = if existing.is_empty() {
        let base = parse_base_hash(base_hash)?;
        if base == REMOTE_LOG_UNKNOWN_BASE_HASH {
            appended
        } else {
            encode_remote_log(page_id, base, &[yrs_update])
        }
    } else {
        appended
    };
    let stored_base = decode_remote_log(page_id, &payload)
        .map_err(map_collab_remote_err)?
        .base_hash;

    let hash = client
        .put_workspace_blob(
            bearer,
            Some(&cloud_workspace.id),
            sidecar_id,
            &payload,
            if_match.as_deref(),
        )
        .map_err(map_err)?;

    Ok(CollabRemoteLogPushResult {
        page_id: page_id.to_string(),
        sidecar_id: sidecar_id.to_string(),
        cloud_workspace_id: cloud_workspace.id,
        content_hash: hash_hex(&hash),
        base_hash: stored_base.to_vec(),
    })
}

/// Pull the remote Yrs append log for `page_resource_id`, if present.
pub fn pull_collab_remote_log(
    root: &str,
    page_resource_id: &str,
) -> Result<Option<CollabRemoteLogPullResult>, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    pull_collab_remote_log_with_client(&api_client(), &bearer, root, page_resource_id)
}

pub fn pull_collab_remote_log_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    page_resource_id: &str,
) -> Result<Option<CollabRemoteLogPullResult>, String> {
    let page_id = parse_doc_resource_id(page_resource_id).map_err(map_err)?;
    let sidecar_id = collab_log_resource_id(page_id);

    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let cloud_workspace = ensure_cloud_workspace(
        client,
        bearer,
        &manifest.id.to_string(),
        manifest.title.as_str(),
    )?;

    let bytes = match client.get_blob(bearer, sidecar_id) {
        Ok(bytes) => bytes,
        Err(CloudError::Api { status: 404, .. }) => return Ok(None),
        Err(err) => return Err(map_err(err)),
    };
    let decoded = decode_remote_log(page_id, &bytes).map_err(map_collab_remote_err)?;
    let hash = ContentHash::from_bytes(&bytes).map_err(map_err)?;

    Ok(Some(CollabRemoteLogPullResult {
        page_id: page_id.to_string(),
        sidecar_id: sidecar_id.to_string(),
        cloud_workspace_id: cloud_workspace.id,
        content_hash: hash_hex(&hash),
        base_hash: decoded.base_hash.to_vec(),
        updates: decoded.updates,
    }))
}

/// Replace the cloud LYRL sidecar blob without appending to the existing log.
///
/// Used after compaction: write a fresh LYRS snapshot, then reset the log with
/// `base_hash` tied to that snapshot and an empty or trimmed update list.
pub fn replace_collab_remote_log(
    root: &str,
    page_resource_id: &str,
    base_hash: Option<&[u8]>,
    updates: &[Vec<u8>],
) -> Result<CollabRemoteLogPushResult, String> {
    let bearer = resolve_cloud_bearer_cmd()?;
    replace_collab_remote_log_with_client(
        &api_client(),
        &bearer,
        root,
        page_resource_id,
        base_hash,
        updates,
    )
}

pub fn replace_collab_remote_log_with_client<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    bearer: &str,
    root: &str,
    page_resource_id: &str,
    base_hash: Option<&[u8]>,
    updates: &[Vec<u8>],
) -> Result<CollabRemoteLogPushResult, String> {
    let page_id = parse_doc_resource_id(page_resource_id).map_err(map_err)?;
    let sidecar_id = collab_log_resource_id(page_id);

    let workspace = Workspace::open(Path::new(root)).map_err(map_err)?;
    let manifest = workspace.manifest();
    let cloud_workspace = ensure_cloud_workspace(
        client,
        bearer,
        &manifest.id.to_string(),
        manifest.title.as_str(),
    )?;

    let existing = match client.get_blob(bearer, sidecar_id) {
        Ok(bytes) => bytes,
        Err(CloudError::Api { status: 404, .. }) => Vec::new(),
        Err(err) => return Err(map_err(err)),
    };
    let if_match = if existing.is_empty() {
        None
    } else {
        let hash = ContentHash::from_bytes(&existing).map_err(map_err)?;
        Some(hash_hex(&hash))
    };

    let base = parse_base_hash(base_hash)?;
    let update_refs: Vec<&[u8]> = updates.iter().map(Vec::as_slice).collect();
    let payload = encode_remote_log(page_id, base, &update_refs);

    let hash = client
        .put_workspace_blob(
            bearer,
            Some(&cloud_workspace.id),
            sidecar_id,
            &payload,
            if_match.as_deref(),
        )
        .map_err(map_err)?;

    Ok(CollabRemoteLogPushResult {
        page_id: page_id.to_string(),
        sidecar_id: sidecar_id.to_string(),
        cloud_workspace_id: cloud_workspace.id,
        content_hash: hash_hex(&hash),
        base_hash: base.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use lattice_cloud_client::{
        CloudApiClient, CloudError, CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse,
        WORKSPACE_ID_HEADER,
    };
    use lattice_collab::{
        collab_log_resource_id, collab_snapshot_resource_id, encode_remote_log,
        encode_remote_snapshot, REMOTE_LOG_MAX_UPDATES, REMOTE_LOG_UNKNOWN_BASE_HASH,
    };
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

    #[test]
    fn push_then_pull_log_roundtrip_via_cloud_blob() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let first = make_text_update("log peer a");
        let second = make_text_update("log peer b");
        let base = [0xab; 32];

        let http = FakeHttp::default();
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let root = dir.path().to_str().unwrap();
        let page_str = page.to_string();

        let pushed = push_collab_remote_log_with_client(
            &client,
            "tok",
            root,
            &page_str,
            &first,
            Some(&base),
        )
        .unwrap();
        assert_eq!(pushed.page_id, page_str);
        assert_eq!(pushed.sidecar_id, collab_log_resource_id(page).to_string());
        assert_eq!(pushed.base_hash, base);

        push_collab_remote_log_with_client(&client, "tok", root, &page_str, &second, Some(&base))
            .unwrap();

        let pulled = pull_collab_remote_log_with_client(&client, "tok", root, &page_str)
            .unwrap()
            .expect("log present");
        assert_eq!(pulled.updates, vec![first, second]);
        assert_eq!(pulled.base_hash, base);
        assert_eq!(pulled.sidecar_id, pushed.sidecar_id);

        let raw = http
            .blobs
            .lock()
            .unwrap()
            .get(&pushed.sidecar_id)
            .cloned()
            .unwrap();
        assert_eq!(
            raw,
            encode_remote_log(
                page,
                base,
                &[pulled.updates[0].as_slice(), pulled.updates[1].as_slice()]
            )
        );
    }

    #[test]
    fn pull_missing_log_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let http = FakeHttp::default();
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let pulled = pull_collab_remote_log_with_client(
            &client,
            "tok",
            dir.path().to_str().unwrap(),
            &page.to_string(),
        )
        .unwrap();
        assert!(pulled.is_none());
    }

    #[test]
    fn push_log_returns_log_needs_compact_at_update_limit() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let sidecar = collab_log_resource_id(page);
        let stubs: Vec<Vec<u8>> = (0..REMOTE_LOG_MAX_UPDATES).map(|i| vec![i as u8]).collect();
        let refs: Vec<&[u8]> = stubs.iter().map(|u| u.as_slice()).collect();
        let payload = encode_remote_log(page, REMOTE_LOG_UNKNOWN_BASE_HASH, &refs);

        let http = FakeHttp::default();
        http.blobs
            .lock()
            .unwrap()
            .insert(sidecar.to_string(), payload);
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");

        let err = push_collab_remote_log_with_client(
            &client,
            "tok",
            dir.path().to_str().unwrap(),
            &page.to_string(),
            &[0xff],
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("log_needs_compact"),
            "expected log_needs_compact in {err}"
        );
    }

    #[test]
    fn push_log_accepts_hex_base_hash() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let update = make_text_update("hex base");
        let hex_base = "11".repeat(32);
        let http = FakeHttp::default();
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");

        let pushed = push_collab_remote_log_with_client(
            &client,
            "tok",
            dir.path().to_str().unwrap(),
            &page.to_string(),
            &update,
            Some(hex_base.as_bytes()),
        )
        .unwrap();
        assert_eq!(pushed.base_hash, vec![0x11; 32]);
    }

    #[test]
    fn replace_log_after_fat_log_then_append_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Demo").unwrap();
        let page = ResourceId::new();
        let sidecar = collab_log_resource_id(page);
        let stubs: Vec<Vec<u8>> = (0..REMOTE_LOG_MAX_UPDATES).map(|i| vec![i as u8]).collect();
        let refs: Vec<&[u8]> = stubs.iter().map(|u| u.as_slice()).collect();
        let fat_payload = encode_remote_log(page, REMOTE_LOG_UNKNOWN_BASE_HASH, &refs);

        let http = FakeHttp::default();
        http.blobs
            .lock()
            .unwrap()
            .insert(sidecar.to_string(), fat_payload);
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let root = dir.path().to_str().unwrap();
        let page_str = page.to_string();

        let new_base = [0xcd; 32];
        let replaced = replace_collab_remote_log_with_client(
            &client,
            "tok",
            root,
            &page_str,
            Some(&new_base),
            &[],
        )
        .unwrap();
        assert_eq!(replaced.base_hash, new_base.to_vec());
        assert_eq!(replaced.sidecar_id, sidecar.to_string());

        let pulled = pull_collab_remote_log_with_client(&client, "tok", root, &page_str)
            .unwrap()
            .expect("log present after replace");
        assert!(pulled.updates.is_empty());
        assert_eq!(pulled.base_hash, new_base);

        let next = make_text_update("after compact");
        push_collab_remote_log_with_client(&client, "tok", root, &page_str, &next, Some(&new_base))
            .unwrap();

        let pulled = pull_collab_remote_log_with_client(&client, "tok", root, &page_str)
            .unwrap()
            .expect("log present after append");
        assert_eq!(pulled.updates, vec![next]);
    }
}
