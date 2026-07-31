//! HTTP contract tests for workspace-local agent thread persistence.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lattice_core::Workspace;
use lattice_daemon::{api_router, daemon_state_for_tests};
use lattice_runtime::LatticeRuntime;
use tempfile::TempDir;
use tower::ServiceExt;

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json body")
}

fn fixture() -> (TempDir, Arc<LatticeRuntime>, String) {
    let dir = TempDir::new().expect("tempdir");
    Workspace::init(dir.path(), "Agent threads HTTP").expect("init");
    let root = dir.path().to_string_lossy().into_owned();
    (dir, Arc::new(LatticeRuntime::new()), root)
}

#[tokio::test]
async fn agent_threads_crud_round_trip_with_token() {
    let (_dir, runtime, root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agent_threads")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "id": "thread-http",
                        "title": "HTTP thread"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_json = body_json(create).await;
    assert_eq!(create_json["thread"]["id"].as_str().unwrap(), "thread-http");
    let workspace_id = create_json["workspaceId"].as_str().unwrap().to_string();

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/agent_threads?workspaceId={workspace_id}"))
                .header("authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_json = body_json(list).await;
    assert_eq!(list_json["threads"].as_array().unwrap().len(), 1);

    let append = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agent_threads/thread-http/messages")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "id": "msg-http",
                        "role": "user",
                        "content": { "type": "text", "text": "hello from http" },
                        "runId": "run-http"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(append.status(), StatusCode::OK);
    let append_json = body_json(append).await;
    assert_eq!(append_json["message"]["id"].as_str().unwrap(), "msg-http");

    let get = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/v1/agent_threads/thread-http?workspaceId={workspace_id}"
                ))
                .header("authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let get_json = body_json(get).await;
    assert_eq!(get_json["messages"].as_array().unwrap().len(), 1);
    assert_eq!(
        get_json["messages"][0]["content"]["text"].as_str().unwrap(),
        "hello from http"
    );

    let db_path = _dir
        .path()
        .join(".lattice/agent/threads.sqlite");
    assert!(db_path.exists());
}

#[tokio::test]
async fn agent_threads_routes_require_auth() {
    let (_dir, runtime, _root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/agent_threads?workspaceId=missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
