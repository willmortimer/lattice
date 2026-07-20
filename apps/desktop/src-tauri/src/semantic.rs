//! Semantic search enable / status / search thin-client for the desktop shell.
//!
//! Production path: Tauri → [`DaemonClient`] → `latticed` → `lattice-embed-host`.
//! Mirrors the voice daemon thin-client pattern without auto-enabling
//! `LATTICE_SEMANTIC_FAKE` (Fake is tests/CI only when already set in the parent).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lattice_client::{request, response, DaemonClient, EventFilter, LatticeClient, Request};
use lattice_handlers::SearchHitUi;
use lattice_protocol::{
    event, DisableSemanticSearchRequest, EnableSemanticSearchRequest, GetSemanticStatusRequest,
    OpenWorkspaceRequest, SearchRequest, SemanticStatus as WireSemanticStatus,
};
#[cfg(test)]
use lattice_runtime::SemanticStatusState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::daemon_session::{self, SpawnHostEnv, SpawnedDaemon};

const SEMANTIC_EVENT: &str = "semantic-event";

const ENV_EMBED_HOST_BIN: &str = "LATTICE_EMBED_HOST_BIN";
const ENV_SEMANTIC_FAKE: &str = "LATTICE_SEMANTIC_FAKE";

#[derive(Default)]
pub struct SemanticState {
    inner: Mutex<SemanticInner>,
}

struct SemanticInner {
    /// Last workspace root used for enable/status (Settings + shell).
    root: Option<String>,
    client: Option<Arc<DaemonClient>>,
    /// Keeps a desktop-spawned daemon alive for the app lifetime.
    /// First spawner (voice or semantic) owns the child; the other attaches via socket.
    _child: Option<SpawnedDaemon>,
    /// OpenWorkspace id keyed by absolute workspace root.
    workspace_ids: HashMap<String, String>,
    /// Forwards SemanticStatusChanged events to the UI (one per connected client).
    forwarder: Option<tokio::task::JoinHandle<()>>,
}

impl Default for SemanticInner {
    fn default() -> Self {
        Self {
            root: None,
            client: None,
            _child: None,
            workspace_ids: HashMap::new(),
            forwarder: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStatusDto {
    pub state: String,
    pub pending_chunks: Option<u64>,
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

impl From<&WireSemanticStatus> for SemanticStatusDto {
    fn from(value: &WireSemanticStatus) -> Self {
        Self {
            state: value.state.clone(),
            pending_chunks: value.pending_chunks,
            message: value.message.clone(),
            progress_percent: value.progress_percent,
            provider_id: value.provider_id.clone(),
            model_id: value.model_id.clone(),
            dimensions: value.dimensions,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SemanticUiEvent {
    #[serde(rename_all = "camelCase")]
    Status {
        state: String,
        pending_chunks: Option<u64>,
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        progress_percent: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimensions: Option<u32>,
    },
}

fn emit_status(app: &AppHandle, status: &SemanticStatusDto) {
    let _ = app.emit(
        SEMANTIC_EVENT,
        SemanticUiEvent::Status {
            state: status.state.clone(),
            pending_chunks: status.pending_chunks,
            message: status.message.clone(),
            progress_percent: status.progress_percent,
            provider_id: status.provider_id.clone(),
            model_id: status.model_id.clone(),
            dimensions: status.dimensions,
        },
    );
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn resolve_embed_host_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_EMBED_HOST_BIN) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(path) = daemon_session::which_bin("lattice-embed-host") {
        return Some(path);
    }
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/lattice-embed-host"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/lattice-embed-host"),
        PathBuf::from("target/debug/lattice-embed-host"),
        PathBuf::from("target/release/lattice-embed-host"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Build spawn env for semantic: may set embed-host bin; never invents SEMANTIC_FAKE.
fn semantic_spawn_host_env() -> SpawnHostEnv {
    let mut extra_env = Vec::new();

    if std::env::var_os(ENV_EMBED_HOST_BIN)
        .filter(|v| !v.is_empty())
        .is_none()
    {
        if let Some(host) = resolve_embed_host_bin() {
            extra_env.push((ENV_EMBED_HOST_BIN.to_string(), host.to_string_lossy().into()));
        }
    }

    // Pass through Fake only when the parent already set it (tests/CI).
    // Never invent LATTICE_SEMANTIC_FAKE=1 here (unlike voice's auto-fake).
    if env_truthy(ENV_SEMANTIC_FAKE) {
        extra_env.push((ENV_SEMANTIC_FAKE.to_string(), "1".into()));
    }

    SpawnHostEnv {
        extra_env,
        handshake_hint: Some(
            "ensure lattice-embed-host is available: build lattice-embed-host, \
             or set LATTICE_EMBED_HOST_BIN",
        ),
    }
}

fn spawn_status_forwarder(app: AppHandle, client: Arc<DaemonClient>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut events = match client
            .subscribe(EventFilter {
                workspace_id: None,
            })
            .await
        {
            Ok(stream) => stream,
            Err(_) => return,
        };
        while let Some(result) = events.next().await {
            let Ok(event) = result else {
                break;
            };
            if let Some(event::Body::SemanticStatus(changed)) = event.body {
                if let Some(status) = changed.status.as_ref() {
                    emit_status(&app, &SemanticStatusDto::from(status));
                }
            }
        }
    })
}

async fn ensure_daemon(
    app: &AppHandle,
    inner: &mut SemanticInner,
) -> Result<Arc<DaemonClient>, String> {
    if let Some(client) = inner.client.as_ref() {
        return Ok(Arc::clone(client));
    }
    let (client, child) = daemon_session::connect_or_spawn(semantic_spawn_host_env()).await?;
    let forwarder = spawn_status_forwarder(app.clone(), Arc::clone(&client));
    inner.client = Some(Arc::clone(&client));
    inner._child = child;
    inner.forwarder = Some(forwarder);
    Ok(client)
}

async fn ensure_workspace(
    client: &DaemonClient,
    inner: &mut SemanticInner,
    root: &str,
) -> Result<String, String> {
    if let Some(id) = inner.workspace_ids.get(root) {
        return Ok(id.clone());
    }
    let opened = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                path: root.to_string(),
            })),
        })
        .await
        .map_err(|err| format!("OpenWorkspace failed: {err}"))?;
    match opened.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            inner
                .workspace_ids
                .insert(root.to_string(), resp.workspace_id.clone());
            Ok(resp.workspace_id)
        }
        other => Err(format!("unexpected OpenWorkspace response: {other:?}")),
    }
}

fn status_from_response(status: Option<WireSemanticStatus>) -> Result<SemanticStatusDto, String> {
    let status = status.ok_or_else(|| "semantic status missing from daemon response".to_string())?;
    Ok(SemanticStatusDto::from(&status))
}

/// True when this process holds an active daemon session for `root` (semantic path).
pub async fn has_daemon_session(state: &SemanticState, root: &str) -> bool {
    let inner = state.inner.lock().await;
    inner.client.is_some() && inner.workspace_ids.contains_key(root)
}

/// Search via latticed when a semantic daemon session is active for `root`.
pub async fn search_via_daemon(
    state: &SemanticState,
    root: &str,
    query: String,
    limit: usize,
    mode: Option<String>,
) -> Result<Option<Vec<SearchHitUi>>, String> {
    let inner = state.inner.lock().await;
    let Some(client) = inner.client.as_ref() else {
        return Ok(None);
    };
    let Some(workspace_id) = inner.workspace_ids.get(root) else {
        return Ok(None);
    };
    let workspace_id = workspace_id.clone();
    let client = Arc::clone(client);
    drop(inner);

    let limit_u32 = u32::try_from(limit).unwrap_or(u32::MAX);
    let searched = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::Search(SearchRequest {
                workspace_id,
                query,
                limit: limit_u32,
                mode,
            })),
        })
        .await
        .map_err(|err| format!("daemon Search failed: {err}"))?;
    match searched.body {
        Some(response::Body::Search(resp)) => Ok(Some(
            resp.hits
                .into_iter()
                .map(|hit| SearchHitUi {
                    path: hit.path,
                    title: hit.title,
                    snippet: hit.snippet,
                    rank: hit.rank,
                    fused_score: hit.fused_score,
                    lexical_rank: hit.lexical_rank,
                    semantic_rank: hit.semantic_rank,
                    heading_path: if hit.heading_path.is_empty() {
                        None
                    } else {
                        Some(hit.heading_path)
                    },
                    chunk_id: hit.chunk_id,
                    sensitivity: hit.sensitivity,
                    export_policy: hit.export_policy,
                })
                .collect(),
        )),
        other => Err(format!("unexpected Search response: {other:?}")),
    }
}

#[tauri::command]
pub async fn semantic_status(
    app: AppHandle,
    root: String,
    state: State<'_, SemanticState>,
) -> Result<SemanticStatusDto, String> {
    let mut inner = state.inner.lock().await;
    inner.root = Some(root.clone());
    let client = ensure_daemon(&app, &mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let responded = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetSemanticStatus(GetSemanticStatusRequest {
                workspace_id,
            })),
        })
        .await
        .map_err(|err| format!("GetSemanticStatus failed: {err}"))?;
    match responded.body {
        Some(response::Body::GetSemanticStatus(resp)) => status_from_response(resp.status),
        other => Err(format!("unexpected GetSemanticStatus response: {other:?}")),
    }
}

#[tauri::command]
pub async fn semantic_enable(
    app: AppHandle,
    root: String,
    state: State<'_, SemanticState>,
) -> Result<SemanticStatusDto, String> {
    let mut inner = state.inner.lock().await;
    inner.root = Some(root.clone());
    let client = ensure_daemon(&app, &mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let responded = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::EnableSemanticSearch(
                EnableSemanticSearchRequest { workspace_id },
            )),
        })
        .await
        .map_err(|err| format!("EnableSemanticSearch failed: {err}"))?;
    let dto = match responded.body {
        Some(response::Body::EnableSemanticSearch(resp)) => status_from_response(resp.status)?,
        other => {
            return Err(format!(
                "unexpected EnableSemanticSearch response: {other:?}"
            ))
        }
    };
    emit_status(&app, &dto);
    Ok(dto)
}

#[tauri::command]
pub async fn semantic_disable(
    app: AppHandle,
    root: String,
    state: State<'_, SemanticState>,
) -> Result<SemanticStatusDto, String> {
    let mut inner = state.inner.lock().await;
    inner.root = Some(root.clone());
    let client = ensure_daemon(&app, &mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let responded = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::DisableSemanticSearch(
                DisableSemanticSearchRequest { workspace_id },
            )),
        })
        .await
        .map_err(|err| format!("DisableSemanticSearch failed: {err}"))?;
    let dto = match responded.body {
        Some(response::Body::DisableSemanticSearch(resp)) => status_from_response(resp.status)?,
        other => {
            return Err(format!(
                "unexpected DisableSemanticSearch response: {other:?}"
            ))
        }
    };
    emit_status(&app, &dto);
    Ok(dto)
}

/// Map a wire/state string into a Settings label (pure helper for tests).
#[cfg(test)]
pub fn status_label(state: &str, pending_chunks: Option<u64>, progress_percent: Option<u32>) -> String {
    let parsed = SemanticStatusState::parse(state);
    match parsed {
        Some(SemanticStatusState::Stopped) => "Not prepared".into(),
        Some(SemanticStatusState::Downloading) => match progress_percent {
            Some(n) => format!("Downloading {n}%"),
            None => "Downloading…".into(),
        },
        Some(SemanticStatusState::Preparing) => "Preparing…".into(),
        Some(SemanticStatusState::Indexing) => match pending_chunks {
            Some(n) if n > 0 => format!("Indexing ({n} pending)"),
            _ => "Indexing…".into(),
        },
        Some(SemanticStatusState::Ready) => "Ready".into(),
        Some(SemanticStatusState::Degraded) => "Degraded (keyword search still works)".into(),
        Some(SemanticStatusState::Failed) => "Failed".into(),
        None => format!("Unknown ({state})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_label_covers_all_states() {
        assert_eq!(status_label("stopped", None, None), "Not prepared");
        assert_eq!(status_label("downloading", None, Some(37)), "Downloading 37%");
        assert_eq!(status_label("preparing", None, None), "Preparing…");
        assert_eq!(status_label("indexing", Some(3), None), "Indexing (3 pending)");
        assert_eq!(status_label("ready", Some(0), None), "Ready");
        assert!(status_label("degraded", None, None).contains("Degraded"));
        assert_eq!(status_label("failed", None, None), "Failed");
    }
}
