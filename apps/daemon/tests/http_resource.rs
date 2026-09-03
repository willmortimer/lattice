//! HTTP auth contract + API-level tests for LatticeFS resource stat and cloud blob open.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use lattice_core::Workspace;
use lattice_daemon::{
    api_cloud_blob_open, api_resource_stat, api_router, daemon_state_for_tests, ApiError,
    ResourcePathParams, WorkspaceRefParams,
};
use lattice_runtime::LatticeRuntime;
use latticefs_core::{materialize_to_cloud, AuthorityMode, InMemoryCloudBlobClient};
use tempfile::TempDir;
use tower::ServiceExt;

fn workspace_params(root: &str) -> WorkspaceRefParams {
    WorkspaceRefParams {
        workspace_id: None,
        root: Some(root.to_string()),
    }
}

fn resource_params(root: &str, path: &str) -> ResourcePathParams {
    ResourcePathParams {
        workspace: workspace_params(root),
        path: path.to_string(),
    }
}

fn fixture() -> (TempDir, Arc<LatticeRuntime>, String) {
    let dir = TempDir::new().expect("tempdir");
    Workspace::init(dir.path(), "HTTP Resource").expect("init");
    std::fs::write(
        dir.path().join("Notes.md"),
        b"# Notes\n\nlocal resource stat test.\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    (dir, Arc::new(LatticeRuntime::new()), root)
}

#[tokio::test]
async fn resource_stat_unauthorized_without_token() {
    let (_dir, runtime, root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/resource/stat")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "path": "Notes.md"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn cloud_blob_open_unauthorized_without_token() {
    let (_dir, runtime, root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cloud/blob_open")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "path": "Notes.md"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn resource_stat_registers_and_returns_local_authority() {
    let (_dir, runtime, root) = fixture();
    let stat = api_resource_stat(&runtime, resource_params(&root, "Notes.md")).expect("stat");
    assert_eq!(stat.path, "Notes.md");
    assert_eq!(stat.authority, AuthorityMode::Local);
}

#[test]
fn cloud_blob_open_fails_closed_for_local_authority() {
    let (_dir, runtime, root) = fixture();
    api_resource_stat(&runtime, resource_params(&root, "Notes.md")).expect("register");
    let err = api_cloud_blob_open(&runtime, resource_params(&root, "Notes.md")).unwrap_err();
    assert!(matches!(err, ApiError::Forbidden(_)));
}

#[test]
fn cloud_blob_open_fails_closed_without_cloud_session() {
    std::env::remove_var("LATTICE_CLOUD_TOKEN");
    let dir = TempDir::new().expect("tempdir");
    Workspace::init(dir.path(), "HTTP Resource").expect("init");
    std::fs::create_dir_all(dir.path().join("notes")).expect("mkdir");
    std::fs::write(dir.path().join("notes/cloud.md"), b"cloud bytes").expect("write");
    let client = InMemoryCloudBlobClient::new();
    materialize_to_cloud(dir.path(), "notes/cloud.md", b"cloud bytes", &client)
        .expect("materialize");
    let root = dir.path().to_string_lossy().into_owned();
    let runtime = Arc::new(LatticeRuntime::new());
    let err = api_cloud_blob_open(&runtime, resource_params(&root, "notes/cloud.md")).unwrap_err();
    assert!(matches!(err, ApiError::Forbidden(_)));
    assert!(
        err.to_string().contains("not signed in to cloud"),
        "unexpected error: {err}"
    );
}
