//! Agent run-event HTTP API (`/v1/agent_runs/*`).
//!
//! Workspace-local durable ordered event log behind latticed; desktop and
//! agents use HTTP, not the DB file. Append/list/status are here; live-tail
//! subscribe is implemented by the desktop thin client (bus wake + list-after-
//! sequence) for gap-free reconnect.

use std::path::PathBuf;

use lattice_runtime::LatticeRuntime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent_run_events_store::{
    AgentRunEventsStore, RunEventRow, RunEventStoreError, RunRow, RunStatus,
};
use crate::agent_threads_api::WorkspaceScopeParams;
use crate::api::{resolve_session, ApiError};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendRunEventParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub thread_id: String,
    pub event_type: String,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub payload_json: Option<String>,
    /// Optional client-supplied idempotency key for this event within the run.
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRunEventsParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    /// Return events with `event_sequence > after_sequence` (default 0).
    #[serde(default)]
    pub after_sequence: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRunStatusParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    /// When set (and `run_id` path empty), resolve the active run for this thread.
    #[serde(default)]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunEventDto {
    pub id: String,
    pub run_id: String,
    pub thread_id: String,
    pub event_sequence: i64,
    pub event_type: String,
    pub payload: Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusDto {
    pub run_id: String,
    pub thread_id: String,
    pub status: RunStatus,
    pub last_sequence: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppendRunEventResponse {
    pub workspace_id: String,
    pub event: RunEventDto,
    pub run: RunStatusDto,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListRunEventsResponse {
    pub workspace_id: String,
    pub run_id: String,
    pub after_sequence: i64,
    pub events: Vec<RunEventDto>,
    pub run: RunStatusDto,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetRunStatusResponse {
    pub workspace_id: String,
    pub run: Option<RunStatusDto>,
}

fn workspace_root_from_session(session: &lattice_runtime::WorkspaceSession) -> PathBuf {
    session.root().to_path_buf()
}

fn map_store_error(err: RunEventStoreError) -> ApiError {
    match err {
        RunEventStoreError::RunNotFound(id) => ApiError::NotFound(format!("run not found: {id}")),
        RunEventStoreError::RunTerminal { run_id, status } => ApiError::BadRequest(format!(
            "run {run_id} is terminal ({status}); cannot append"
        )),
        RunEventStoreError::InvalidStatus(status) => {
            ApiError::Internal(format!("invalid run status in store: {status}"))
        }
        other => ApiError::Internal(other.to_string()),
    }
}

fn run_dto(row: RunRow) -> RunStatusDto {
    RunStatusDto {
        run_id: row.run_id,
        thread_id: row.thread_id,
        status: row.status,
        last_sequence: row.last_sequence,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn event_dto(row: RunEventRow) -> RunEventDto {
    let payload = serde_json::from_str(&row.payload_json).unwrap_or(Value::Null);
    RunEventDto {
        id: row.id,
        run_id: row.run_id,
        thread_id: row.thread_id,
        event_sequence: row.event_sequence,
        event_type: row.event_type,
        payload,
        created_at: row.created_at,
    }
}

fn resolve_payload_json(
    payload: Option<Value>,
    payload_json: Option<String>,
) -> Result<String, ApiError> {
    match (payload, payload_json) {
        (Some(value), _) => serde_json::to_string(&value)
            .map_err(|err| ApiError::BadRequest(format!("payload must be JSON: {err}"))),
        (None, Some(raw)) if !raw.trim().is_empty() => Ok(raw),
        (None, Some(_)) => Ok("{}".into()),
        (None, None) => Ok("{}".into()),
    }
}

fn open_store(
    runtime: &LatticeRuntime,
    scope: &WorkspaceScopeParams,
) -> Result<(String, AgentRunEventsStore), ApiError> {
    let session = resolve_session(
        runtime,
        scope.workspace_id.as_deref(),
        scope.root.as_deref(),
    )?;
    let workspace_id = session.workspace_id().to_string();
    let root = workspace_root_from_session(&session);
    let store = AgentRunEventsStore::open(&root).map_err(map_store_error)?;
    Ok((workspace_id, store))
}

/// Append a durable ordered run event (assigns next monotonic sequence).
pub fn api_append_run_event(
    runtime: &LatticeRuntime,
    run_id: &str,
    params: AppendRunEventParams,
) -> Result<AppendRunEventResponse, ApiError> {
    if run_id.trim().is_empty() {
        return Err(ApiError::BadRequest("run id is required".into()));
    }
    if params.thread_id.trim().is_empty() {
        return Err(ApiError::BadRequest("threadId is required".into()));
    }
    if params.event_type.trim().is_empty() {
        return Err(ApiError::BadRequest("eventType is required".into()));
    }
    let payload_json = resolve_payload_json(params.payload, params.payload_json)?;
    let scope = WorkspaceScopeParams {
        workspace_id: params.workspace_id,
        root: params.root,
    };
    let (workspace_id, mut store) = open_store(runtime, &scope)?;
    let event = store
        .append_event(
            run_id,
            params.thread_id.trim(),
            params.event_type.trim(),
            &payload_json,
            params.id,
        )
        .map_err(map_store_error)?;
    let run = store
        .get_run(run_id)
        .map_err(map_store_error)?
        .ok_or_else(|| ApiError::Internal(format!("run missing after append: {run_id}")))?;
    Ok(AppendRunEventResponse {
        workspace_id,
        event: event_dto(event),
        run: run_dto(run),
    })
}

/// List events with sequence greater than `after_sequence` (replay cursor).
pub fn api_list_run_events(
    runtime: &LatticeRuntime,
    run_id: &str,
    params: ListRunEventsParams,
) -> Result<ListRunEventsResponse, ApiError> {
    if run_id.trim().is_empty() {
        return Err(ApiError::BadRequest("run id is required".into()));
    }
    let after_sequence = params.after_sequence.unwrap_or(0).max(0);
    let scope = WorkspaceScopeParams {
        workspace_id: params.workspace_id,
        root: params.root,
    };
    let (workspace_id, store) = open_store(runtime, &scope)?;
    let run = store
        .get_run(run_id)
        .map_err(map_store_error)?
        .ok_or_else(|| ApiError::NotFound(format!("run not found: {run_id}")))?;
    let events = store
        .list_events_after(run_id, after_sequence)
        .map_err(map_store_error)?
        .into_iter()
        .map(event_dto)
        .collect();
    Ok(ListRunEventsResponse {
        workspace_id,
        run_id: run_id.to_string(),
        after_sequence,
        events,
        run: run_dto(run),
    })
}

/// Get run status by run id, or the active run for a thread when `thread_id` is set.
pub fn api_get_run_status(
    runtime: &LatticeRuntime,
    run_id: Option<&str>,
    params: GetRunStatusParams,
) -> Result<GetRunStatusResponse, ApiError> {
    let scope = WorkspaceScopeParams {
        workspace_id: params.workspace_id,
        root: params.root,
    };
    let (workspace_id, store) = open_store(runtime, &scope)?;

    let run = if let Some(id) = run_id.filter(|id| !id.trim().is_empty()) {
        store
            .get_run(id)
            .map_err(map_store_error)?
            .ok_or_else(|| ApiError::NotFound(format!("run not found: {id}")))
            .map(Some)?
    } else if let Some(thread_id) = params
        .thread_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
    {
        store
            .get_active_run_for_thread(thread_id)
            .map_err(map_store_error)?
    } else {
        return Err(ApiError::BadRequest(
            "run id path or threadId query is required".into(),
        ));
    };

    Ok(GetRunStatusResponse {
        workspace_id,
        run: run.map(run_dto),
    })
}

#[cfg(test)]
mod tests {
    use lattice_core::Workspace;
    use lattice_runtime::LatticeRuntime;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn fixture() -> (TempDir, LatticeRuntime, String) {
        let dir = TempDir::new().expect("tempdir");
        Workspace::init(dir.path(), "Agent run events").expect("init");
        let root = dir.path().to_string_lossy().into_owned();
        (dir, LatticeRuntime::new(), root)
    }

    #[test]
    fn api_append_list_status_round_trip() {
        let (_dir, runtime, root) = fixture();

        let appended = api_append_run_event(
            &runtime,
            "run-api",
            AppendRunEventParams {
                workspace_id: None,
                root: Some(root.clone()),
                thread_id: "thread-api".into(),
                event_type: "message_chunk".into(),
                payload: Some(json!({ "type": "text-delta", "delta": "hello" })),
                payload_json: None,
                id: Some("evt-1".into()),
            },
        )
        .expect("append");
        assert_eq!(appended.event.event_sequence, 1);
        assert_eq!(appended.run.status, RunStatus::Running);

        let _ = api_append_run_event(
            &runtime,
            "run-api",
            AppendRunEventParams {
                workspace_id: None,
                root: Some(root.clone()),
                thread_id: "thread-api".into(),
                event_type: "message_chunk".into(),
                payload: Some(json!({ "type": "text-delta", "delta": " world" })),
                payload_json: None,
                id: Some("evt-2".into()),
            },
        )
        .expect("append 2");

        let listed = api_list_run_events(
            &runtime,
            "run-api",
            ListRunEventsParams {
                workspace_id: None,
                root: Some(root.clone()),
                after_sequence: Some(1),
            },
        )
        .expect("list");
        assert_eq!(listed.events.len(), 1);
        assert_eq!(listed.events[0].id, "evt-2");
        assert_eq!(listed.events[0].event_sequence, 2);

        let status = api_get_run_status(
            &runtime,
            Some("run-api"),
            GetRunStatusParams {
                workspace_id: None,
                root: Some(root.clone()),
                thread_id: None,
            },
        )
        .expect("status");
        assert_eq!(status.run.as_ref().unwrap().last_sequence, 2);

        let by_thread = api_get_run_status(
            &runtime,
            None,
            GetRunStatusParams {
                workspace_id: None,
                root: Some(root),
                thread_id: Some("thread-api".into()),
            },
        )
        .expect("by thread");
        assert_eq!(by_thread.run.as_ref().unwrap().run_id, "run-api");
    }

    #[test]
    fn api_idempotent_append() {
        let (_dir, runtime, root) = fixture();
        let params = AppendRunEventParams {
            workspace_id: None,
            root: Some(root),
            thread_id: "t".into(),
            event_type: "message_chunk".into(),
            payload: Some(json!({"n": 1})),
            payload_json: None,
            id: Some("same".into()),
        };
        let a = api_append_run_event(&runtime, "run-x", params.clone()).expect("a");
        let b = api_append_run_event(&runtime, "run-x", params).expect("b");
        assert_eq!(a.event, b.event);
        assert_eq!(a.run.last_sequence, 1);
        assert_eq!(b.run.last_sequence, 1);
    }
}
