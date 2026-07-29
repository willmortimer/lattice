//! Loopback HTTP `/mcp` contract tests for MCP 2026-07-28.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lattice_core::Workspace;
use lattice_daemon::{api_router, daemon_state_for_tests};
use lattice_mcp_catalog::{TOOL_WORKSPACE_READ, TOOL_WORKSPACE_SEARCH};
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
    Workspace::init(dir.path(), "HTTP MCP").expect("init");
    std::fs::write(
        dir.path().join("Notes.md"),
        "# Notes\n\nUnique http-mcp-phrase for search.\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    (dir, Arc::new(LatticeRuntime::new()), root)
}

#[tokio::test]
async fn mcp_requires_auth() {
    let (_dir, runtime, _root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "tools/list",
                        "params": {}
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
async fn mcp_tools_list_returns_catalog_names() {
    let (_dir, runtime, _root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .header("mcp-protocol-version", "2026-07-28")
                .header("mcp-method", "tools/list")
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "tools/list",
                        "params": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let names: Vec<&str> = json["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names[0], TOOL_WORKSPACE_SEARCH);
    assert!(names.iter().all(|n| n.starts_with("workspace.")));
}

#[tokio::test]
async fn mcp_tools_call_read_round_trip() {
    // Prefer read over search here: FTS/index open can block indefinitely under
    // the async oneshot runtime in this harness; search is covered by mcp unit tests.
    let (_dir, runtime, root) = fixture();
    let app = api_router(daemon_state_for_tests("secret-token", runtime));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret-token")
                .header("mcp-protocol-version", "2026-07-28")
                .header("mcp-method", "tools/call")
                .header("mcp-name", TOOL_WORKSPACE_READ)
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/call",
                        "params": {
                            "name": TOOL_WORKSPACE_READ,
                            "arguments": {
                                "root": root,
                                "path": "Notes.md",
                                "max_bytes": 4096
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["result"]["isError"], false);
    let text = json["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("http-mcp-phrase") || text.contains("Notes"));
}
