//! Push/pull executor: apply planner output via lattice-cloud-client.
//!
//! Capture, presence, and app lock remain out of scope (S4 fence).

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_cloud_client::{CloudApiClient, CloudHttpClient, WorkspaceSyncHead};
use lattice_storage::atomic_write_file;
use latticefs_core::{ContentHash, NamespaceRegistry, ResourceId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::sync_state::{SyncState, SyncStateError};
use crate::{
    normalize_content_hash, plan, LocalSnapshotEntry, PlanEntry, SyncHead, SyncStatus,
};

pub type Result<T> = std::result::Result<T, ExecutorError>;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("json error at {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("registry error: {0}")]
    Registry(#[from] latticefs_core::Error),
    #[error("sync-state error: {0}")]
    SyncState(#[from] SyncStateError),
    #[error("cloud error: {0}")]
    Cloud(#[from] lattice_cloud_client::CloudError),
    #[error("invalid resource id {value}: {source}")]
    InvalidResourceId {
        value: String,
        source: uuid::Error,
    },
    #[error("missing local path for resource {resource_id}")]
    MissingPath { resource_id: String },
    #[error("local file missing for push at {path}")]
    MissingLocalFile { path: PathBuf },
    #[error("resource {resource_id} not found in sync plan")]
    NotInPlan { resource_id: String },
    #[error("resource {resource_id} is not conflicted (status: {status:?})")]
    NotConflicted {
        resource_id: String,
        status: SyncStatus,
    },
    #[error("cloud head changed during conflict resolve (409) for {resource_id}")]
    ConflictStale { resource_id: String },
}

/// Explicit user choice for a planner [`SyncStatus::Conflicted`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Push local bytes over cloud with `If-Match` = current cloud head.
    KeepLocal,
    /// Pull cloud blob bytes over the local file.
    TakeCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteOutcome {
    NoOp,
    Pushed,
    Pulled,
    SkippedConflicted,
    KeptLocal,
    TookCloud,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResult {
    pub resource_id: String,
    pub status: SyncStatus,
    pub outcome: ExecuteOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunReport {
    pub cloud_workspace_id: String,
    pub results: Vec<ExecuteResult>,
}

/// Build planner local snapshot rows from on-disk files plus sync-state metadata.
pub fn local_snapshot_from_workspace(
    workspace_root: &Path,
    registry: &NamespaceRegistry,
    sync_state: &SyncState,
) -> Result<Vec<LocalSnapshotEntry>> {
    let mut entries = Vec::new();
    for entry in registry.entries() {
        let full_path = workspace_root.join(&entry.path);
        if !full_path.is_file() {
            continue;
        }
        let bytes = fs::read(&full_path).map_err(|source| ExecutorError::Io {
            path: full_path.clone(),
            source,
        })?;
        let hash = ContentHash::from_bytes(&bytes)?;
        let resource_id = entry.resource_id.to_string();
        entries.push(LocalSnapshotEntry {
            resource_id,
            content_hash: hash.as_str().to_string(),
            path: Some(entry.path),
            last_synced_hash: sync_state
                .last_synced_hash(&entry.resource_id.to_string())
                .map(str::to_owned),
        });
    }
    Ok(entries)
}

fn sync_heads_from_cloud(heads: Vec<WorkspaceSyncHead>) -> Vec<SyncHead> {
    heads
        .into_iter()
        .map(|head| SyncHead {
            resource_id: head.resource_id,
            content_hash: head.content_hash,
            updated_at: head.updated_at,
        })
        .collect()
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn resolve_path_owned(
    entry: &PlanEntry,
    registry: &NamespaceRegistry,
    sync_state: &SyncState,
) -> Result<String> {
    if let Some(path) = entry.path.clone() {
        return Ok(path);
    }
    if let Some(path) = sync_state.path_for(&entry.resource_id) {
        return Ok(path.to_string());
    }
    let resource_id = ResourceId::from_str(&entry.resource_id).map_err(|source| {
        ExecutorError::InvalidResourceId {
            value: entry.resource_id.clone(),
            source,
        }
    })?;
    registry
        .path_for_resource_id(resource_id)
        .ok_or_else(|| ExecutorError::MissingPath {
            resource_id: entry.resource_id.clone(),
        })
}

/// Apply one planner entry against cloud + local workspace state.
pub fn execute_plan_entry<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    workspace_root: &Path,
    cloud_workspace_id: &str,
    bearer: &str,
    registry: &mut NamespaceRegistry,
    sync_state: &mut SyncState,
    entry: &PlanEntry,
) -> ExecuteResult {
    match entry.status {
        SyncStatus::InSync => ExecuteResult {
            resource_id: entry.resource_id.clone(),
            status: entry.status,
            outcome: ExecuteOutcome::NoOp,
            content_hash: entry.local_hash.clone(),
            error: None,
        },
        SyncStatus::Conflicted => ExecuteResult {
            resource_id: entry.resource_id.clone(),
            status: entry.status,
            outcome: ExecuteOutcome::SkippedConflicted,
            content_hash: entry.local_hash.clone().or_else(|| entry.cloud_hash.clone()),
            error: None,
        },
        SyncStatus::MissingCloud | SyncStatus::Dirty => {
            match execute_push(
                client,
                workspace_root,
                cloud_workspace_id,
                bearer,
                registry,
                sync_state,
                entry,
            ) {
                Ok(hash_hex) => ExecuteResult {
                    resource_id: entry.resource_id.clone(),
                    status: entry.status,
                    outcome: ExecuteOutcome::Pushed,
                    content_hash: Some(hash_hex),
                    error: None,
                },
                Err(err) => ExecuteResult {
                    resource_id: entry.resource_id.clone(),
                    status: entry.status,
                    outcome: ExecuteOutcome::Failed,
                    content_hash: None,
                    error: Some(err.to_string()),
                },
            }
        }
        SyncStatus::MissingLocal => {
            match execute_pull(
                client,
                workspace_root,
                bearer,
                registry,
                sync_state,
                entry,
            ) {
                Ok(hash_hex) => ExecuteResult {
                    resource_id: entry.resource_id.clone(),
                    status: entry.status,
                    outcome: ExecuteOutcome::Pulled,
                    content_hash: Some(hash_hex),
                    error: None,
                },
                Err(err) => ExecuteResult {
                    resource_id: entry.resource_id.clone(),
                    status: entry.status,
                    outcome: ExecuteOutcome::Failed,
                    content_hash: None,
                    error: Some(err.to_string()),
                },
            }
        }
    }
}

fn execute_push<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    workspace_root: &Path,
    cloud_workspace_id: &str,
    bearer: &str,
    registry: &mut NamespaceRegistry,
    sync_state: &mut SyncState,
    entry: &PlanEntry,
) -> Result<String> {
    let path = resolve_path_owned(entry, registry, sync_state)?;
    let full_path = workspace_root.join(&path);
    let bytes = fs::read(&full_path).map_err(|source| ExecutorError::Io {
        path: full_path.clone(),
        source,
    })?;
    if bytes.is_empty() {
        return Err(ExecutorError::MissingLocalFile { path: full_path });
    }
    let resource_id = ResourceId::from_str(&entry.resource_id).map_err(|source| {
        ExecutorError::InvalidResourceId {
            value: entry.resource_id.clone(),
            source,
        }
    })?;
    let if_match = entry.cloud_hash.as_deref();
    let hash = client.put_workspace_blob(
        bearer,
        Some(cloud_workspace_id),
        resource_id,
        &bytes,
        if_match,
    )?;
    let hash_hex = normalize_content_hash(hash.as_str());
    registry.set_content_hash(&path, hash.clone())?;
    registry.mark_cloud_backed(&path, hash)?;
    sync_state.record_success(
        &entry.resource_id,
        Some(&path),
        &hash_hex,
        Some(now_unix()),
    );
    Ok(hash_hex)
}

fn execute_pull<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    workspace_root: &Path,
    bearer: &str,
    registry: &mut NamespaceRegistry,
    sync_state: &mut SyncState,
    entry: &PlanEntry,
) -> Result<String> {
    let path = resolve_path_owned(entry, registry, sync_state)?;
    let resource_id = ResourceId::from_str(&entry.resource_id).map_err(|source| {
        ExecutorError::InvalidResourceId {
            value: entry.resource_id.clone(),
            source,
        }
    })?;
    let bytes = client.get_blob(bearer, resource_id)?;
    let hash = ContentHash::from_bytes(&bytes)?;
    let hash_hex = normalize_content_hash(hash.as_str());
    if let Some(cloud_hash) = entry.cloud_hash.as_deref() {
        if hash_hex != cloud_hash {
            return Err(ExecutorError::Registry(latticefs_core::Error::BlobHashMismatch {
                expected: ContentHash::new(format!("sha256:{cloud_hash}"))?,
                actual: hash.clone(),
            }));
        }
    }
    let full_path = workspace_root.join(&path);
    atomic_write_file(&full_path, &bytes).map_err(|err| ExecutorError::Io {
        path: full_path,
        source: std::io::Error::new(std::io::ErrorKind::Other, err.to_string()),
    })?;
    if registry.path_for_resource_id(resource_id).is_none() {
        registry.ensure_local_file(&path)?;
    }
    registry.set_content_hash(&path, hash.clone())?;
    registry.mark_cloud_backed(&path, hash)?;
    sync_state.record_success(
        &entry.resource_id,
        Some(&path),
        &hash_hex,
        entry.cloud_updated_at.or(Some(now_unix())),
    );
    Ok(hash_hex)
}

fn map_push_cloud_error(resource_id: &str, err: lattice_cloud_client::CloudError) -> ExecutorError {
    if err.api_status() == Some(409) {
        ExecutorError::ConflictStale {
            resource_id: resource_id.to_string(),
        }
    } else {
        ExecutorError::Cloud(err)
    }
}

fn execute_push_mapping_conflict<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    workspace_root: &Path,
    cloud_workspace_id: &str,
    bearer: &str,
    registry: &mut NamespaceRegistry,
    sync_state: &mut SyncState,
    entry: &PlanEntry,
) -> Result<String> {
    match execute_push(
        client,
        workspace_root,
        cloud_workspace_id,
        bearer,
        registry,
        sync_state,
        entry,
    ) {
        Ok(hash) => Ok(hash),
        Err(ExecutorError::Cloud(err)) => Err(map_push_cloud_error(&entry.resource_id, err)),
        Err(err) => Err(err),
    }
}

/// Resolve a single conflicted plan row: keep local (push) or take cloud (pull).
///
/// Loads sync-heads + planner output, requires the resource to be
/// [`SyncStatus::Conflicted`], applies the chosen side, and persists registry +
/// sync-state (same bookkeeping as a normal push/pull).
///
/// A cloud `409` on keep-local becomes [`ExecutorError::ConflictStale`].
pub fn resolve_conflict<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    workspace_root: &Path,
    cloud_workspace_id: &str,
    bearer: &str,
    resource_id: &str,
    resolution: ConflictResolution,
) -> Result<ExecuteResult> {
    let mut registry = NamespaceRegistry::open(workspace_root)?;
    let mut sync_state = SyncState::open(workspace_root)?;
    let local = local_snapshot_from_workspace(workspace_root, &registry, &sync_state)?;
    let cloud_heads = client.get_sync_heads(bearer, cloud_workspace_id)?;
    let plan_entries = plan(&local, &sync_heads_from_cloud(cloud_heads));
    let entry = plan_entries
        .iter()
        .find(|entry| entry.resource_id == resource_id)
        .cloned()
        .ok_or_else(|| ExecutorError::NotInPlan {
            resource_id: resource_id.to_string(),
        })?;
    if entry.status != SyncStatus::Conflicted {
        return Err(ExecutorError::NotConflicted {
            resource_id: resource_id.to_string(),
            status: entry.status,
        });
    }

    let (outcome, hash_hex) = match resolution {
        ConflictResolution::KeepLocal => {
            let hash_hex = execute_push_mapping_conflict(
                client,
                workspace_root,
                cloud_workspace_id,
                bearer,
                &mut registry,
                &mut sync_state,
                &entry,
            )?;
            (ExecuteOutcome::KeptLocal, hash_hex)
        }
        ConflictResolution::TakeCloud => {
            let hash_hex = execute_pull(
                client,
                workspace_root,
                bearer,
                &mut registry,
                &mut sync_state,
                &entry,
            )?;
            (ExecuteOutcome::TookCloud, hash_hex)
        }
    };

    registry.save()?;
    sync_state.save()?;
    Ok(ExecuteResult {
        resource_id: entry.resource_id,
        status: SyncStatus::InSync,
        outcome,
        content_hash: Some(hash_hex),
        error: None,
    })
}

/// Fetch sync-heads, plan, execute, and persist registry + sync-state.
pub fn run_workspace_sync<C: CloudHttpClient>(
    client: &CloudApiClient<C>,
    workspace_root: &Path,
    cloud_workspace_id: &str,
    bearer: &str,
) -> Result<SyncRunReport> {
    let mut registry = NamespaceRegistry::open(workspace_root)?;
    let mut sync_state = SyncState::open(workspace_root)?;
    let local = local_snapshot_from_workspace(workspace_root, &registry, &sync_state)?;
    let cloud_heads = client.get_sync_heads(bearer, cloud_workspace_id)?;
    let plan_entries = plan(&local, &sync_heads_from_cloud(cloud_heads));
    let mut results = Vec::with_capacity(plan_entries.len());
    for entry in &plan_entries {
        results.push(execute_plan_entry(
            client,
            workspace_root,
            cloud_workspace_id,
            bearer,
            &mut registry,
            &mut sync_state,
            entry,
        ));
    }
    registry.save()?;
    sync_state.save()?;
    Ok(SyncRunReport {
        cloud_workspace_id: cloud_workspace_id.to_string(),
        results,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use lattice_cloud_client::{
        CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse, CloudError,
        IF_MATCH_HEADER, WORKSPACE_ID_HEADER,
    };
    use serde_json::Value;

    use super::*;

    #[derive(Default, Clone)]
    struct SyncFakeHttp {
        json: Arc<Mutex<HashMap<String, CloudHttpResponse>>>,
        bytes: Arc<Mutex<HashMap<String, CloudHttpBytesResponse>>>,
        blobs: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        heads: Arc<Mutex<Vec<WorkspaceSyncHead>>>,
    }

    impl CloudHttpClient for SyncFakeHttp {
        fn request(
            &self,
            _base_url: &str,
            method: &str,
            path: &str,
            _body: Option<&Value>,
            bearer: Option<&str>,
        ) -> lattice_cloud_client::Result<CloudHttpResponse> {
            if bearer != Some("good-token") {
                return Ok(CloudHttpResponse {
                    status: 401,
                    body: r#"{"error":"invalid session"}"#.into(),
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
        ) -> lattice_cloud_client::Result<CloudHttpBytesResponse> {
            if bearer != Some("good-token") {
                return Ok(CloudHttpBytesResponse {
                    status: 401,
                    body: br#"{"error":"invalid session"}"#.to_vec(),
                    content_hash: None,
                });
            }
            let key = format!("{method} {path}");
            if method == "PUT" {
                let data = body.expect("put body");
                let hash = ContentHash::from_bytes(data).expect("hash");
                let hash_hex = normalize_content_hash(hash.as_str());
                let resource_id = path.trim_start_matches("/v1/blobs/");
                if let Some((_, expected)) = headers.iter().find(|(name, _)| *name == IF_MATCH_HEADER)
                {
                    let expected = expected.trim().trim_matches('"').to_ascii_lowercase();
                    let expected = expected
                        .strip_prefix("sha256:")
                        .unwrap_or(expected.as_str());
                    let current = self
                        .heads
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|head| head.resource_id == resource_id)
                        .map(|head| head.content_hash.clone());
                    if current.as_deref() != Some(expected) {
                        return Ok(CloudHttpBytesResponse {
                            status: 409,
                            body: br#"{"error":"if-match conflict"}"#.to_vec(),
                            content_hash: None,
                        });
                    }
                }
                self.blobs
                    .lock()
                    .unwrap()
                    .insert(resource_id.to_string(), data.to_vec());
                if headers
                    .iter()
                    .any(|(name, _)| *name == WORKSPACE_ID_HEADER)
                {
                    self.heads.lock().unwrap().retain(|head| head.resource_id != resource_id);
                    self.heads.lock().unwrap().push(WorkspaceSyncHead {
                        resource_id: resource_id.to_string(),
                        content_hash: hash_hex.clone(),
                        updated_at: 1,
                    });
                }
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
            if method == "GET" && path.starts_with("/v1/blobs/") {
                let resource_id = path.trim_start_matches("/v1/blobs/");
                let data = self
                    .blobs
                    .lock()
                    .unwrap()
                    .get(resource_id)
                    .cloned()
                    .ok_or_else(|| CloudError::Http(format!("blob missing for {resource_id}")))?;
                let hash_hex = normalize_content_hash(
                    ContentHash::from_bytes(&data)
                        .expect("hash")
                        .as_str(),
                );
                return Ok(CloudHttpBytesResponse {
                    status: 200,
                    body: data,
                    content_hash: Some(hash_hex),
                });
            }
            self.bytes
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| CloudError::Http(format!("no fake bytes for {key}")))
        }
    }

    fn setup_workspace_with_file(content: &[u8]) -> (tempfile::TempDir, ResourceId, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = "notes/a.md";
        let full_path = dir.path().join(path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(&full_path, content).unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        let resource_id = registry.ensure_local_file(path).unwrap();
        registry.save().unwrap();
        (dir, resource_id, path.to_string())
    }

    #[test]
    fn push_then_pull_round_trip_updates_registry_hash() {
        let (dir, resource_id, path) = setup_workspace_with_file(b"hello-sync");
        let http = SyncFakeHttp::default();
        http.json.lock().unwrap().insert(
            "GET /v1/workspaces/cloud-ws/sync-heads".into(),
            CloudHttpResponse {
                status: 200,
                body: "[]".into(),
            },
        );
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");

        let push_report = run_workspace_sync(
            &client,
            dir.path(),
            "cloud-ws",
            "good-token",
        )
        .unwrap();
        assert_eq!(push_report.results.len(), 1);
        assert_eq!(push_report.results[0].outcome, ExecuteOutcome::Pushed);

        let pushed_hash = push_report.results[0]
            .content_hash
            .clone()
            .expect("hash");
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat(&path).unwrap();
        assert_eq!(
            normalize_content_hash(stat.content_hash.as_ref().unwrap().as_str()),
            pushed_hash
        );

        fs::remove_file(dir.path().join(&path)).unwrap();
        http.json.lock().unwrap().insert(
            "GET /v1/workspaces/cloud-ws/sync-heads".into(),
            CloudHttpResponse {
                status: 200,
                body: format!(
                    r#"[{{"resource_id":"{resource_id}","content_hash":"{pushed_hash}","updated_at":2}}]"#
                ),
            },
        );
        let pull_report = run_workspace_sync(
            &client,
            dir.path(),
            "cloud-ws",
            "good-token",
        )
        .unwrap();
        assert_eq!(pull_report.results.len(), 1);
        assert_eq!(pull_report.results[0].outcome, ExecuteOutcome::Pulled);
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat(&path).unwrap();
        assert_eq!(
            normalize_content_hash(stat.content_hash.as_ref().unwrap().as_str()),
            pushed_hash
        );
        assert!(dir.path().join(&path).is_file());
    }

    #[test]
    fn conflicted_skips_write() {
        let (dir, resource_id, path) = setup_workspace_with_file(b"local-copy");
        let local_hash = normalize_content_hash(
            ContentHash::from_bytes(b"local-copy")
                .unwrap()
                .as_str(),
        );
        let cloud_hash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let http = SyncFakeHttp::default();
        http.json.lock().unwrap().insert(
            "GET /v1/workspaces/cloud-ws/sync-heads".into(),
            CloudHttpResponse {
                status: 200,
                body: format!(
                    r#"[{{"resource_id":"{resource_id}","content_hash":"{cloud_hash}","updated_at":1}}]"#
                ),
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let report = run_workspace_sync(&client, dir.path(), "cloud-ws", "good-token").unwrap();
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].status, SyncStatus::Conflicted);
        assert_eq!(report.results[0].outcome, ExecuteOutcome::SkippedConflicted);
        assert_eq!(
            fs::read(dir.path().join(&path)).unwrap(),
            b"local-copy"
        );
        assert_ne!(local_hash, cloud_hash);
    }

    fn setup_conflicted(
        local_bytes: &[u8],
        cloud_bytes: &[u8],
    ) -> (tempfile::TempDir, ResourceId, String, String, SyncFakeHttp) {
        let (dir, resource_id, path) = setup_workspace_with_file(local_bytes);
        let cloud_hash = normalize_content_hash(
            ContentHash::from_bytes(cloud_bytes).unwrap().as_str(),
        );
        let http = SyncFakeHttp::default();
        http.blobs
            .lock()
            .unwrap()
            .insert(resource_id.to_string(), cloud_bytes.to_vec());
        http.heads.lock().unwrap().push(WorkspaceSyncHead {
            resource_id: resource_id.to_string(),
            content_hash: cloud_hash.clone(),
            updated_at: 1,
        });
        http.json.lock().unwrap().insert(
            "GET /v1/workspaces/cloud-ws/sync-heads".into(),
            CloudHttpResponse {
                status: 200,
                body: format!(
                    r#"[{{"resource_id":"{resource_id}","content_hash":"{cloud_hash}","updated_at":1}}]"#
                ),
            },
        );
        (dir, resource_id, path, cloud_hash, http)
    }

    #[test]
    fn resolve_keep_local_pushes_with_if_match() {
        let (dir, resource_id, path, cloud_hash, http) =
            setup_conflicted(b"local-wins", b"cloud-copy");
        let client = CloudApiClient::with_base_url(http.clone(), "https://cloud.test");
        let result = resolve_conflict(
            &client,
            dir.path(),
            "cloud-ws",
            "good-token",
            &resource_id.to_string(),
            ConflictResolution::KeepLocal,
        )
        .unwrap();
        assert_eq!(result.outcome, ExecuteOutcome::KeptLocal);
        assert_eq!(result.status, SyncStatus::InSync);
        let local_hash = normalize_content_hash(
            ContentHash::from_bytes(b"local-wins").unwrap().as_str(),
        );
        assert_eq!(result.content_hash.as_deref(), Some(local_hash.as_str()));
        assert_ne!(local_hash, cloud_hash);
        assert_eq!(
            http.blobs.lock().unwrap().get(&resource_id.to_string()).unwrap(),
            b"local-wins"
        );
        let sync_state = SyncState::open(dir.path()).unwrap();
        assert_eq!(
            sync_state.last_synced_hash(&resource_id.to_string()),
            Some(local_hash.as_str())
        );
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat(&path).unwrap();
        assert_eq!(
            normalize_content_hash(stat.content_hash.as_ref().unwrap().as_str()),
            local_hash
        );
    }

    #[test]
    fn resolve_take_cloud_overwrites_local_file() {
        let (dir, resource_id, path, cloud_hash, http) =
            setup_conflicted(b"local-copy", b"cloud-wins");
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let result = resolve_conflict(
            &client,
            dir.path(),
            "cloud-ws",
            "good-token",
            &resource_id.to_string(),
            ConflictResolution::TakeCloud,
        )
        .unwrap();
        assert_eq!(result.outcome, ExecuteOutcome::TookCloud);
        assert_eq!(result.status, SyncStatus::InSync);
        assert_eq!(result.content_hash.as_deref(), Some(cloud_hash.as_str()));
        assert_eq!(fs::read(dir.path().join(&path)).unwrap(), b"cloud-wins");
        let sync_state = SyncState::open(dir.path()).unwrap();
        assert_eq!(
            sync_state.last_synced_hash(&resource_id.to_string()),
            Some(cloud_hash.as_str())
        );
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat(&path).unwrap();
        assert_eq!(
            normalize_content_hash(stat.content_hash.as_ref().unwrap().as_str()),
            cloud_hash
        );
    }

    #[test]
    fn resolve_keep_local_409_is_conflict_stale() {
        let (dir, resource_id, _path, _cloud_hash, http) =
            setup_conflicted(b"local-wins", b"cloud-copy");
        // Advance the fake head so If-Match against the plan's cloud hash fails.
        let drifted = normalize_content_hash(
            ContentHash::from_bytes(b"drifted-cloud").unwrap().as_str(),
        );
        http.heads.lock().unwrap().clear();
        http.heads.lock().unwrap().push(WorkspaceSyncHead {
            resource_id: resource_id.to_string(),
            content_hash: drifted.clone(),
            updated_at: 2,
        });
        // Planner still sees the old head from sync-heads JSON.
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let err = resolve_conflict(
            &client,
            dir.path(),
            "cloud-ws",
            "good-token",
            &resource_id.to_string(),
            ConflictResolution::KeepLocal,
        )
        .unwrap_err();
        assert!(
            matches!(err, ExecutorError::ConflictStale { .. }),
            "expected ConflictStale, got {err:?}"
        );
    }

    #[test]
    fn resolve_rejects_non_conflicted_resource() {
        let (dir, resource_id, _path) = setup_workspace_with_file(b"only-local");
        let http = SyncFakeHttp::default();
        http.json.lock().unwrap().insert(
            "GET /v1/workspaces/cloud-ws/sync-heads".into(),
            CloudHttpResponse {
                status: 200,
                body: "[]".into(),
            },
        );
        let client = CloudApiClient::with_base_url(http, "https://cloud.test");
        let err = resolve_conflict(
            &client,
            dir.path(),
            "cloud-ws",
            "good-token",
            &resource_id.to_string(),
            ConflictResolution::KeepLocal,
        )
        .unwrap_err();
        assert!(
            matches!(err, ExecutorError::NotConflicted { status: SyncStatus::MissingCloud, .. }),
            "expected NotConflicted, got {err:?}"
        );
    }
}
