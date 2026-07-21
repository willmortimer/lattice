//! HTTP contract tests for the localhost governed context API.

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
    Workspace::init(dir.path(), "HTTP API").expect("init");
    std::fs::write(
        dir.path().join("Notes.md"),
        "---\nexport_policy: allow\n---\n\n# Notes\n\nUnique http-api-phrase for search.\n",
    )
    .expect("write");
    std::fs::write(
        dir.path().join("Link.md"),
        "---\nexport_policy: allow\n---\n\n# Link\n\nSee [[Notes]] here.\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    (dir, Arc::new(LatticeRuntime::new()), root)
}

#[tokio::test]
async fn unauthorized_without_token() {
    let (_dir, runtime, root) = fixture();
    let state = daemon_state_for_tests("secret-token", runtime);
    let app = api_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/search")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "query": "http-api-phrase"
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
async fn unauthorized_with_wrong_token() {
    let (_dir, runtime, root) = fixture();
    let state = daemon_state_for_tests("secret-token", runtime);
    let app = api_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/search")
                .header("content-type", "application/json")
                .header("authorization", "Bearer wrong")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "query": "http-api-phrase"
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
async fn search_and_read_round_trip_with_token() {
    let (_dir, runtime, root) = fixture();
    let state = daemon_state_for_tests("secret-token", Arc::clone(&runtime));
    let app = api_router(state);

    let search = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/search")
                .header("content-type", "application/json")
                .header("x-lattice-token", "secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "query": "http-api-phrase",
                        "mode": "fts",
                        "limit": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    let search_json = body_json(search).await;
    assert!(!search_json["hits"].as_array().unwrap().is_empty());
    let workspace_id = search_json["workspaceId"].as_str().unwrap().to_string();

    let read = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/read")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "workspaceId": workspace_id,
                        "path": "Notes.md",
                        "maxBytes": 128
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    let read_json = body_json(read).await;
    assert!(read_json["content"]
        .as_str()
        .unwrap()
        .contains("http-api-phrase"));
    assert!(read_json["revision"].as_str().unwrap().starts_with("sha256:"));
}

#[tokio::test]
async fn related_and_build_context_require_auth() {
    let (_dir, runtime, root) = fixture();
    let state = daemon_state_for_tests("secret-token", runtime);
    let app = api_router(state);

    let related = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/related")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
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
    assert_eq!(related.status(), StatusCode::OK);
    let related_json = body_json(related).await;
    assert!(related_json["hits"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["path"].as_str().unwrap().contains("Link")));

    let ctx = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/build_context")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "query": "http-api-phrase",
                        "limit": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ctx.status(), StatusCode::OK);
    let ctx_json = body_json(ctx).await;
    assert!(
        ctx_json["excerpts"].as_array().unwrap().len()
            + ctx_json["omittedAskOrDeny"].as_u64().unwrap_or(0) as usize
            > 0
    );
}

#[tokio::test]
async fn health_is_open() {
    let state = daemon_state_for_tests("secret-token", Arc::new(LatticeRuntime::new()));
    let app = api_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn proposal_create_list_get_round_trip() {
    let (_dir, runtime, root) = fixture();
    let state = daemon_state_for_tests("secret-token", runtime);
    let app = api_router(state);

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/proposals/propose_page")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "root": root,
                        "path": "Proposals/HTTP.md",
                        "content": "# HTTP proposal\n"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_json = body_json(create).await;
    let proposal_id = create_json["proposal"]["id"].as_str().unwrap().to_string();
    let workspace_id = create_json["workspaceId"].as_str().unwrap().to_string();
    assert_eq!(create_json["proposal"]["source"]["type"].as_str().unwrap(), "mcp");

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/proposals/list")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({ "workspaceId": workspace_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_json = body_json(list).await;
    assert_eq!(list_json["proposals"].as_array().unwrap().len(), 1);
    assert_eq!(
        list_json["proposals"][0]["id"].as_str().unwrap(),
        proposal_id
    );

    let get = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/proposals/get")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .body(Body::from(
                    serde_json::json!({
                        "workspaceId": workspace_id,
                        "proposalId": proposal_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let get_json = body_json(get).await;
    assert_eq!(
        get_json["proposal"]["commands"][0]["type"].as_str().unwrap(),
        "page-create"
    );
}

#[tokio::test]
async fn proposal_routes_require_auth() {
    let (_dir, runtime, root) = fixture();
    let state = daemon_state_for_tests("secret-token", runtime);
    let app = api_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/proposals/list")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({ "root": root }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
