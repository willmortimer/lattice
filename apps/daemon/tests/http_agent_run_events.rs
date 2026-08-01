//! HTTP contract tests for durable agent run-event log.

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
    Workspace::init(dir.path(), "Agent run events HTTP").expect("init");
    let root = dir.path().to_string_lossy().into_owned();
    (dir, Arc::new(LatticeRuntime::new()), root)
}

#[tokio::test]
async fn agent_run_events_append_list_status_with_token() {
    let (_dir, runtime, root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let append = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agent_runs/run-http/events")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "threadId": "thread-http",
                        "eventType": "message_chunk",
                        "payload": { "type": "text-delta", "delta": "hi" },
                        "id": "evt-http-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(append.status(), StatusCode::OK);
    let append_json = body_json(append).await;
    assert_eq!(append_json["event"]["eventSequence"].as_i64(), Some(1));
    let workspace_id = append_json["workspaceId"].as_str().unwrap().to_string();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agent_runs/run-http/events")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "workspaceId": workspace_id,
                        "threadId": "thread-http",
                        "eventType": "run_completed",
                        "payload": { "type": "run_completed" },
                        "id": "evt-http-2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/v1/agent_runs/run-http/events?workspaceId={workspace_id}&afterSequence=0"
                ))
                .header("authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_json = body_json(list).await;
    assert_eq!(list_json["events"].as_array().unwrap().len(), 2);
    assert_eq!(list_json["run"]["status"].as_str(), Some("completed"));

    let after = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/v1/agent_runs/run-http/events?workspaceId={workspace_id}&afterSequence=1"
                ))
                .header("authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let after_json = body_json(after).await;
    assert_eq!(after_json["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        after_json["events"][0]["eventType"].as_str(),
        Some("run_completed")
    );

    let status = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/v1/agent_runs/run-http?workspaceId={workspace_id}"
                ))
                .header("authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_json = body_json(status).await;
    assert_eq!(status_json["run"]["lastSequence"].as_i64(), Some(2));
}

#[tokio::test]
async fn agent_run_events_routes_require_auth() {
    let (_dir, runtime, _root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/agent_runs/missing?workspaceId=missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
