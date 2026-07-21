//! Authenticated localhost HTTP surface for the governed context API.
//!
//! Binds **127.0.0.1 only**. All `/v1/*` routes require the daemon auth token
//! via `Authorization: Bearer <token>` or `X-Lattice-Token`.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use lattice_runtime::LatticeRuntime;
use serde::Serialize;
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::api::{
    api_build_context, api_create_proposal, api_get_proposal, api_list_proposals, api_propose_page,
    api_read, api_related, api_search, ApiError, BuildContextParams, CreateProposalParams,
    GetProposalParams, ListProposalsParams, ProposePageParams, ReadParams, RelatedParams,
    SearchParams,
};
use crate::config::DaemonConfig;
use crate::server::DaemonState;

const AUTH_HEADER: &str = "x-lattice-token";

#[derive(Clone)]
struct HttpState {
    daemon: DaemonState,
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code().to_string(),
                    message: self.message().to_string(),
                },
            }),
        )
            .into_response()
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: ErrorDetail {
                code: "unauthorized".into(),
                message: "invalid or missing auth token".into(),
            },
        }),
    )
        .into_response()
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(AUTH_HEADER).and_then(|v| v.to_str().ok()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let bearer = auth.strip_prefix("Bearer ").or_else(|| auth.strip_prefix("bearer "))?;
    let trimmed = bearer.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn require_auth(state: &HttpState, headers: &HeaderMap) -> Result<(), Response> {
    match extract_token(headers) {
        Some(token) if token == state.daemon.config.auth_token => Ok(()),
        _ => Err(unauthorized()),
    }
}

async fn health(State(state): State<HttpState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "instanceId": state.daemon.config.instance_id,
        "protocol": "lattice-local-api/v1",
    }))
}

async fn route_search(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<SearchParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_search(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_read(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<ReadParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_read(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_related(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<RelatedParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_related(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_build_context(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<BuildContextParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_build_context(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_create_proposal(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<CreateProposalParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_create_proposal(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_list_proposals(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<ListProposalsParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_list_proposals(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_get_proposal(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<GetProposalParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_get_proposal(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn route_propose_page(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<ProposePageParams>,
) -> Response {
    if let Err(resp) = require_auth(&state, &headers) {
        return resp;
    }
    match api_propose_page(&state.daemon.runtime, body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(err) => err.into_response(),
    }
}

/// Build the localhost API router (no CORS — not a browser demo surface).
pub fn router(daemon: DaemonState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/search", post(route_search))
        .route("/v1/read", post(route_read))
        .route("/v1/related", post(route_related))
        .route("/v1/build_context", post(route_build_context))
        .route("/v1/proposals/create", post(route_create_proposal))
        .route("/v1/proposals/list", post(route_list_proposals))
        .route("/v1/proposals/get", post(route_get_proposal))
        .route("/v1/proposals/propose_page", post(route_propose_page))
        .with_state(HttpState { daemon })
}

/// Bind `127.0.0.1:port` and serve until `shutdown` fires.
pub async fn serve_localhost_api(
    daemon: DaemonState,
    port: u16,
    shutdown: oneshot::Receiver<()>,
) -> crate::Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    info!(%bound, "latticed local API listening (loopback only)");

    let app = router(daemon);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
            info!("latticed local API shutting down");
        })
        .await
        .map_err(crate::Error::Io)?;
    Ok(())
}

/// Spawn the localhost API on a background task; returns a shutdown sender.
pub fn spawn_localhost_api(daemon: DaemonState, port: u16) -> oneshot::Sender<()> {
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        if let Err(err) = serve_localhost_api(daemon, port, rx).await {
            warn!(error = %err, "local API exited with error");
        }
    });
    tx
}

/// Helper for tests: bind an ephemeral port and return `(addr, shutdown_tx, join)`.
pub async fn serve_localhost_api_ephemeral(
    daemon: DaemonState,
) -> crate::Result<(SocketAddr, oneshot::Sender<()>, tokio::task::JoinHandle<crate::Result<()>>)> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let (tx, rx) = oneshot::channel();
    let app = router(daemon);
    let join = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
            .map_err(crate::Error::Io)?;
        Ok(())
    });
    Ok((bound, tx, join))
}

/// Shared runtime handle for constructing [`DaemonState`] in API-only tests.
pub fn daemon_state_for_tests(
    auth_token: impl Into<String>,
    runtime: Arc<LatticeRuntime>,
) -> DaemonState {
    let config = DaemonConfig::new("/tmp/latticed-api-test.sock", auth_token).with_api_port(None);
    DaemonState::new(config, runtime)
}
