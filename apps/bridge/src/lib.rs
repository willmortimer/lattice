//! Localhost HTTP surface over [`lattice_handlers`].
//!
//! Each route maps 1:1 to a handler function so the browser demo can call the
//! same logic as Tauri without duplicating domain code.

use std::net::SocketAddr;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use lattice_handlers::{
    apply_page_update, create_page, create_workspace, ensure_home, get_backlinks, list_resources,
    list_templates, open_workspace, read_page, rebuild_index, search_workspace, STALE_REVISION_PREFIX,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

/// Shared server configuration passed to every route handler.
#[derive(Debug, Clone)]
pub struct BridgeState {
    pub default_root: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenWorkspaceRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootRequest {
    #[serde(default)]
    root: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadPageRequest {
    #[serde(default)]
    root: Option<String>,
    rel_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPageUpdateRequest {
    #[serde(default)]
    root: Option<String>,
    rel_path: String,
    content: String,
    base_revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePageRequest {
    #[serde(default)]
    root: Option<String>,
    rel_path: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    template_path: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchWorkspaceRequest {
    #[serde(default)]
    root: Option<String>,
    query: String,
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize {
    25
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BacklinksRequest {
    #[serde(default)]
    root: Option<String>,
    rel_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkspaceRequest {
    path: String,
    #[serde(default)]
    title: Option<String>,
    template: String,
    #[serde(default)]
    set_default: bool,
    #[serde(default)]
    initialize_existing: bool,
}

fn resolve_root(state: &BridgeState, root: Option<String>) -> Result<String, Response> {
    root.or_else(|| state.default_root.clone())
        .ok_or_else(|| handler_error("workspace root is required (pass root or start with --root)".into()))
}

fn handler_error(message: String) -> Response {
    let status = if message.starts_with(STALE_REVISION_PREFIX) {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(ErrorBody { error: message })).into_response()
}

fn handler_result<T: Serialize>(result: Result<T, String>) -> Response {
    match result {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(message) => handler_error(message),
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn route_open_workspace(
    State(state): State<BridgeState>,
    Json(body): Json<OpenWorkspaceRequest>,
) -> Response {
    let _ = &state;
    handler_result(open_workspace(body.path))
}

async fn route_list_resources(
    State(state): State<BridgeState>,
    Json(body): Json<RootRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    handler_result(list_resources(root))
}

async fn route_read_page(
    State(state): State<BridgeState>,
    Json(body): Json<ReadPageRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    handler_result(read_page(root, body.rel_path))
}

async fn route_apply_page_update(
    State(state): State<BridgeState>,
    Json(body): Json<ApplyPageUpdateRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    match apply_page_update(root, body.rel_path, body.content, body.base_revision) {
        Ok(revision) => (StatusCode::OK, Json(serde_json::json!({ "revision": revision }))).into_response(),
        Err(message) => handler_error(message),
    }
}

async fn route_create_page(
    State(state): State<BridgeState>,
    Json(body): Json<CreatePageRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    match create_page(
        root,
        body.rel_path,
        body.content,
        body.template_path,
        body.title,
    ) {
        Ok(revision) => (StatusCode::OK, Json(serde_json::json!({ "revision": revision }))).into_response(),
        Err(message) => handler_error(message),
    }
}

async fn route_search_workspace(
    State(state): State<BridgeState>,
    Json(body): Json<SearchWorkspaceRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    handler_result(search_workspace(root, body.query, body.limit))
}

async fn route_rebuild_index(
    State(state): State<BridgeState>,
    Json(body): Json<RootRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    match rebuild_index(root) {
        Ok(pages_indexed) => (
            StatusCode::OK,
            Json(serde_json::json!({ "pagesIndexed": pages_indexed })),
        )
            .into_response(),
        Err(message) => handler_error(message),
    }
}

async fn route_get_backlinks(
    State(state): State<BridgeState>,
    Json(body): Json<BacklinksRequest>,
) -> Response {
    let root = match resolve_root(&state, body.root) {
        Ok(root) => root,
        Err(response) => return response,
    };
    handler_result(get_backlinks(root, body.rel_path))
}

async fn route_ensure_home() -> Response {
    handler_result(ensure_home())
}

async fn route_list_templates() -> Json<serde_json::Value> {
    Json(serde_json::to_value(list_templates()).expect("templates serialize"))
}

async fn route_create_workspace(Json(body): Json<CreateWorkspaceRequest>) -> Response {
    handler_result(create_workspace(
        body.path,
        body.title,
        body.template,
        body.set_default,
        body.initialize_existing,
    ))
}

/// Build the axum router with CORS for the Vite dev server.
pub fn router(state: BridgeState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse().expect("localhost origin"),
            "http://127.0.0.1:5173".parse().expect("127.0.0.1 origin"),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/open_workspace", post(route_open_workspace))
        .route("/list_resources", post(route_list_resources))
        .route("/read_page", post(route_read_page))
        .route("/apply_page_update", post(route_apply_page_update))
        .route("/create_page", post(route_create_page))
        .route("/search_workspace", post(route_search_workspace))
        .route("/rebuild_index", post(route_rebuild_index))
        .route("/get_backlinks", post(route_get_backlinks))
        .route("/ensure_home", post(route_ensure_home))
        .route("/list_templates", post(route_list_templates))
        .route("/create_workspace", post(route_create_workspace))
        .layer(cors)
        .with_state(state)
}

/// Bind and serve the bridge until the process is interrupted.
pub async fn serve(host: &str, port: u16, state: BridgeState) -> std::io::Result<()> {
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(std::io::Error::other)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("lattice-bridge listening on http://{addr}");
    axum::serve(listener, router(state)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use lattice_core::Workspace;
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

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Bridge Test").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        dir
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = router(BridgeState {
            default_root: None,
        });
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
        let json = body_json(response).await;
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn open_workspace_and_read_page_round_trip() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let app = router(BridgeState {
            default_root: Some(root.clone()),
        });

        let open = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/open_workspace")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "path": root }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(open.status(), StatusCode::OK);
        let snapshot = body_json(open).await;
        assert_eq!(snapshot["title"], "Bridge Test");

        let page = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/read_page")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "relPath": "Notes.md" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(page.status(), StatusCode::OK);
        let page_json = body_json(page).await;
        assert_eq!(page_json["content"], "# Hi\n");
        assert!(page_json["revision"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[tokio::test]
    async fn missing_root_without_default_is_bad_request() {
        let app = router(BridgeState {
            default_root: None,
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/read_page")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "relPath": "Notes.md" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
