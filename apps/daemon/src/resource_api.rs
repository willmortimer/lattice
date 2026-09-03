//! LatticeFS resource stat and cloud blob open for the governed HTTP/MCP API.

use std::path::{Component, Path};

use base64::Engine;
use lattice_cloud_client::{
    default_client, process_cloud_session_store, resolve_cloud_bearer, CloudSessionStore,
    HttpCloudBlobClient,
};
use latticefs_core::{
    open_cloud_authoritative_bytes, resource_stat, resource_stat_or_register, AuthorityMode,
    CloudBlobClient, ResourceStat,
};
use serde::Serialize;

use crate::api::ApiError;

fn session_store() -> &'static dyn CloudSessionStore {
    process_cloud_session_store()
}

fn validate_rel_path(rel_path: &str) -> Result<String, ApiError> {
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return Err(ApiError::BadRequest(format!(
            "{rel_path:?} must be relative to the workspace root"
        )));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ApiError::BadRequest(format!(
            "{rel_path:?} escapes the workspace root"
        )));
    }
    let trimmed = rel_path.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest("path must not be empty".into()));
    }
    Ok(trimmed.replace('\\', "/"))
}

fn map_fs_error(err: latticefs_core::Error) -> ApiError {
    match err {
        latticefs_core::Error::ResourceNotFound { path } => {
            ApiError::NotFound(format!("resource not found at path: {path}"))
        }
        latticefs_core::Error::OutsideWorkspace { path } => {
            ApiError::BadRequest(format!("path escapes workspace: {}", path.display()))
        }
        latticefs_core::Error::NotCloudAuthoritative { path } => {
            ApiError::Forbidden(format!("resource is not cloud-authoritative: {path}"))
        }
        latticefs_core::Error::CloudBlob { message } => {
            ApiError::Forbidden(format!("cloud blob error: {message}"))
        }
        latticefs_core::Error::BlobNotFound { resource_id } => {
            ApiError::NotFound(format!("blob not found for resource: {resource_id}"))
        }
        other => ApiError::Internal(other.to_string()),
    }
}

/// Inspect authority and materialization for a workspace-relative path.
pub fn resource_stat_at(root: &Path, path: &str) -> Result<ResourceStat, ApiError> {
    let rel_key = validate_rel_path(path)?;
    resource_stat_or_register(root, &rel_key).map_err(map_fs_error)
}

/// Response for `cloud_blob_open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBlobOpenResponse {
    pub workspace_id: String,
    pub path: String,
    pub bytes_base64: String,
}

/// Fetch canonical bytes for a cloud-backed resource via the given blob client.
pub fn open_cloud_blob_at(
    root: &Path,
    workspace_id: &str,
    path: &str,
    client: &dyn CloudBlobClient,
) -> Result<CloudBlobOpenResponse, ApiError> {
    let rel_key = validate_rel_path(path)?;
    let bytes = open_cloud_authoritative_bytes(root, &rel_key, client).map_err(map_fs_error)?;
    Ok(CloudBlobOpenResponse {
        workspace_id: workspace_id.to_string(),
        path: rel_key,
        bytes_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// Fetch canonical bytes using the signed-in cloud session (fails closed when unsigned).
pub fn open_cloud_blob_with_session(
    root: &Path,
    workspace_id: &str,
    path: &str,
) -> Result<CloudBlobOpenResponse, ApiError> {
    let rel_key = validate_rel_path(path)?;
    let stat = resource_stat(root, &rel_key).map_err(map_fs_error)?;
    if stat.authority != AuthorityMode::Cloud {
        return Err(ApiError::Forbidden(format!(
            "resource is not cloud-authoritative: {}",
            stat.path
        )));
    }
    let token = resolve_cloud_bearer(session_store())
        .map_err(|err| ApiError::Forbidden(err.to_string()))?;
    let client = HttpCloudBlobClient::new(default_client(), token);
    open_cloud_blob_at(root, workspace_id, path, &client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use latticefs_core::{
        materialize_to_cloud, AuthorityMode, InMemoryCloudBlobClient, NamespaceRegistry,
    };
    use tempfile::tempdir;

    #[test]
    fn resource_stat_registers_local_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), b"hello").unwrap();
        let stat = resource_stat_at(dir.path(), "note.md").unwrap();
        assert_eq!(stat.path, "note.md");
        assert_eq!(stat.authority, AuthorityMode::Local);
        assert!(NamespaceRegistry::open(dir.path())
            .unwrap()
            .resource_stat("note.md")
            .is_ok());
    }

    #[test]
    fn open_cloud_blob_rejects_local_authority() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), b"local").unwrap();
        resource_stat_at(dir.path(), "note.md").unwrap();
        let client = InMemoryCloudBlobClient::new();
        let err = open_cloud_blob_at(dir.path(), "ws-test", "note.md", &client).unwrap_err();
        assert!(matches!(err, ApiError::Forbidden(_)));
    }

    #[test]
    fn open_cloud_blob_returns_cloud_bytes() {
        let dir = tempdir().unwrap();
        let data = b"cloud-canonical";
        let client = InMemoryCloudBlobClient::new();
        materialize_to_cloud(dir.path(), "notes/a.md", data, &client).unwrap();
        let response = open_cloud_blob_at(dir.path(), "ws-test", "notes/a.md", &client).unwrap();
        assert_eq!(response.path, "notes/a.md");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(response.bytes_base64)
            .unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn open_cloud_blob_with_session_fails_when_unsigned() {
        let dir = tempdir().unwrap();
        let data = b"cloud-canonical";
        let client = InMemoryCloudBlobClient::new();
        materialize_to_cloud(dir.path(), "notes/a.md", data, &client).unwrap();
        std::env::remove_var("LATTICE_CLOUD_TOKEN");
        let err = open_cloud_blob_with_session(dir.path(), "ws-test", "notes/a.md").unwrap_err();
        assert!(matches!(err, ApiError::Forbidden(_)));
    }
}
