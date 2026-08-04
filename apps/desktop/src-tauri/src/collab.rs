//! Collaborative page editing thin-client for the desktop shell.
//!
//! Tauri → [`DaemonClient`] → `latticed` → in-memory Yrs sessions (Y0).

use std::collections::HashMap;
use std::sync::Arc;

use lattice_client::{request, response, DaemonClient, LatticeClient, Request};
use lattice_protocol::{
    ApplyCollabUpdateRequest, CloseCollabDocRequest, GetCollabStateRequest, OpenCollabDocRequest,
    OpenWorkspaceRequest,
};
use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::daemon_session::{self, SpawnHostEnv, SpawnedDaemon};

#[derive(Default)]
pub struct CollabState {
    inner: Mutex<CollabInner>,
}

struct CollabInner {
    client: Option<Arc<DaemonClient>>,
    _child: Option<SpawnedDaemon>,
    workspace_ids: HashMap<String, String>,
}

impl Default for CollabInner {
    fn default() -> Self {
        Self {
            client: None,
            _child: None,
            workspace_ids: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCollabDocDto {
    pub doc_id: String,
    pub state_vector: Vec<u8>,
    pub update: Vec<u8>,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCollabUpdateDto {
    pub doc_id: String,
    pub state_vector: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCollabStateDto {
    pub doc_id: String,
    pub state_vector: Vec<u8>,
    pub update: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseCollabDocDto {
    pub closed: bool,
}

async fn ensure_client(inner: &mut CollabInner) -> Result<Arc<DaemonClient>, String> {
    if let Some(client) = inner.client.as_ref() {
        return Ok(Arc::clone(client));
    }
    let (client, child) = daemon_session::connect_or_spawn(SpawnHostEnv::default()).await?;
    inner.client = Some(Arc::clone(&client));
    inner._child = child;
    Ok(client)
}

async fn ensure_workspace(
    client: &DaemonClient,
    inner: &mut CollabInner,
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

#[tauri::command]
pub async fn open_collab_doc(
    state: State<'_, CollabState>,
    root: String,
    doc_id: String,
    path: Option<String>,
) -> Result<OpenCollabDocDto, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_client(&mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let opened = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::OpenCollabDoc(OpenCollabDocRequest {
                workspace_id,
                doc_id,
                path,
            })),
        })
        .await
        .map_err(|err| format!("OpenCollabDoc failed: {err}"))?;

    match opened.body {
        Some(response::Body::OpenCollabDoc(resp)) => Ok(OpenCollabDocDto {
            doc_id: resp.doc_id,
            state_vector: resp.state_vector,
            update: resp.update,
            created: resp.created,
        }),
        other => Err(format!("unexpected OpenCollabDoc response: {other:?}")),
    }
}

#[tauri::command]
pub async fn apply_collab_update(
    state: State<'_, CollabState>,
    root: String,
    doc_id: String,
    update: Vec<u8>,
) -> Result<ApplyCollabUpdateDto, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_client(&mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let applied = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::ApplyCollabUpdate(ApplyCollabUpdateRequest {
                workspace_id,
                doc_id,
                update,
            })),
        })
        .await
        .map_err(|err| format!("ApplyCollabUpdate failed: {err}"))?;

    match applied.body {
        Some(response::Body::ApplyCollabUpdate(resp)) => Ok(ApplyCollabUpdateDto {
            doc_id: resp.doc_id,
            state_vector: resp.state_vector,
        }),
        other => Err(format!("unexpected ApplyCollabUpdate response: {other:?}")),
    }
}

#[tauri::command]
pub async fn get_collab_state(
    state: State<'_, CollabState>,
    root: String,
    doc_id: String,
    state_vector: Vec<u8>,
) -> Result<GetCollabStateDto, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_client(&mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let snapshot = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetCollabState(GetCollabStateRequest {
                workspace_id,
                doc_id,
                state_vector,
            })),
        })
        .await
        .map_err(|err| format!("GetCollabState failed: {err}"))?;

    match snapshot.body {
        Some(response::Body::GetCollabState(resp)) => Ok(GetCollabStateDto {
            doc_id: resp.doc_id,
            state_vector: resp.state_vector,
            update: resp.update,
        }),
        other => Err(format!("unexpected GetCollabState response: {other:?}")),
    }
}

#[tauri::command]
pub async fn close_collab_doc(
    state: State<'_, CollabState>,
    root: String,
    doc_id: String,
) -> Result<CloseCollabDocDto, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_client(&mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let closed = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::CloseCollabDoc(CloseCollabDocRequest {
                workspace_id,
                doc_id,
            })),
        })
        .await
        .map_err(|err| format!("CloseCollabDoc failed: {err}"))?;

    match closed.body {
        Some(response::Body::CloseCollabDoc(resp)) => Ok(CloseCollabDocDto {
            closed: resp.closed,
        }),
        other => Err(format!("unexpected CloseCollabDoc response: {other:?}")),
    }
}
