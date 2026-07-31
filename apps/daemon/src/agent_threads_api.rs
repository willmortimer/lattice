//! Agent thread HTTP API (`/v1/agent_threads/*`).
//!
//! Workspace-local SQLite behind latticed; desktop and agents use HTTP, not the DB file.

use std::path::PathBuf;

use lattice_runtime::LatticeRuntime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::{resolve_session, ApiError};
use crate::agent_threads_store::{
    AgentThreadsStore, MessageRow, ThreadRow, ThreadStoreError,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceScopeParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateThreadParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendMessageParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    pub role: String,
    #[serde(default)]
    pub content: Option<Value>,
    #[serde(default)]
    pub content_json: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDto {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content: Value,
    pub run_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListThreadsResponse {
    pub workspace_id: String,
    pub threads: Vec<ThreadDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateThreadResponse {
    pub workspace_id: String,
    pub thread: ThreadDto,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetThreadResponse {
    pub workspace_id: String,
    pub thread: ThreadDto,
    pub messages: Vec<MessageDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppendMessageResponse {
    pub workspace_id: String,
    pub message: MessageDto,
}

fn workspace_root_from_session(session: &lattice_runtime::WorkspaceSession) -> PathBuf {
    session.root().to_path_buf()
}

fn map_store_error(err: ThreadStoreError) -> ApiError {
    match err {
        ThreadStoreError::ThreadNotFound(id) => ApiError::NotFound(format!("thread not found: {id}")),
        other => ApiError::Internal(other.to_string()),
    }
}

fn thread_dto(row: ThreadRow) -> ThreadDto {
    ThreadDto {
        id: row.id,
        title: row.title,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn message_dto(row: MessageRow) -> Result<MessageDto, ApiError> {
    let content = serde_json::from_str(&row.content_json).unwrap_or(Value::Null);
    Ok(MessageDto {
        id: row.id,
        thread_id: row.thread_id,
        role: row.role,
        content,
        run_id: row.run_id,
        created_at: row.created_at,
    })
}

fn resolve_content_json(
    content: Option<Value>,
    content_json: Option<String>,
) -> Result<String, ApiError> {
    match (content, content_json) {
        (Some(value), _) => serde_json::to_string(&value)
            .map_err(|err| ApiError::BadRequest(format!("content must be JSON: {err}"))),
        (None, Some(raw)) if !raw.trim().is_empty() => Ok(raw),
        _ => Err(ApiError::BadRequest("content or contentJson is required".into())),
    }
}

fn open_store(runtime: &LatticeRuntime, scope: &WorkspaceScopeParams) -> Result<(String, AgentThreadsStore), ApiError> {
    let session = resolve_session(
        runtime,
        scope.workspace_id.as_deref(),
        scope.root.as_deref(),
    )?;
    let workspace_id = session.workspace_id().to_string();
    let root = workspace_root_from_session(&session);
    let store = AgentThreadsStore::open(&root).map_err(map_store_error)?;
    Ok((workspace_id, store))
}

/// List workspace-local agent threads (metadata only).
pub fn api_list_threads(
    runtime: &LatticeRuntime,
    params: WorkspaceScopeParams,
) -> Result<ListThreadsResponse, ApiError> {
    let (workspace_id, store) = open_store(runtime, &params)?;
    let threads = store
        .list_threads()
        .map_err(map_store_error)?
        .into_iter()
        .map(thread_dto)
        .collect();
    Ok(ListThreadsResponse {
        workspace_id,
        threads,
    })
}

/// Create a workspace-local agent thread.
pub fn api_create_thread(
    runtime: &LatticeRuntime,
    params: CreateThreadParams,
) -> Result<CreateThreadResponse, ApiError> {
    let scope = WorkspaceScopeParams {
        workspace_id: params.workspace_id,
        root: params.root,
    };
    let (workspace_id, mut store) = open_store(runtime, &scope)?;
    let thread_id = params
        .id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let thread = store
        .create_thread(Some(thread_id), params.title)
        .map_err(map_store_error)?;
    Ok(CreateThreadResponse {
        workspace_id,
        thread: thread_dto(thread),
    })
}

/// Fetch a thread and its messages.
pub fn api_get_thread(
    runtime: &LatticeRuntime,
    thread_id: &str,
    params: WorkspaceScopeParams,
) -> Result<GetThreadResponse, ApiError> {
    if thread_id.trim().is_empty() {
        return Err(ApiError::BadRequest("thread id is required".into()));
    }
    let (workspace_id, store) = open_store(runtime, &params)?;
    let thread = store
        .get_thread(thread_id)
        .map_err(map_store_error)?
        .ok_or_else(|| ApiError::NotFound(format!("thread not found: {thread_id}")))?;
    let messages = store
        .list_messages(thread_id)
        .map_err(map_store_error)?
        .into_iter()
        .map(message_dto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GetThreadResponse {
        workspace_id,
        thread: thread_dto(thread),
        messages,
    })
}

/// Append a message to a workspace-local agent thread.
pub fn api_append_message(
    runtime: &LatticeRuntime,
    thread_id: &str,
    params: AppendMessageParams,
) -> Result<AppendMessageResponse, ApiError> {
    if thread_id.trim().is_empty() {
        return Err(ApiError::BadRequest("thread id is required".into()));
    }
    if params.role.trim().is_empty() {
        return Err(ApiError::BadRequest("role must not be empty".into()));
    }
    let content_json = resolve_content_json(params.content, params.content_json)?;
    let scope = WorkspaceScopeParams {
        workspace_id: params.workspace_id,
        root: params.root,
    };
    let (workspace_id, mut store) = open_store(runtime, &scope)?;
    let message = store
        .append_message(
            thread_id,
            params.id,
            params.role.trim(),
            &content_json,
            params.run_id,
        )
        .map_err(map_store_error)?;
    Ok(AppendMessageResponse {
        workspace_id,
        message: message_dto(message)?,
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
        Workspace::init(dir.path(), "Agent threads").expect("init");
        let root = dir.path().to_string_lossy().into_owned();
        (dir, LatticeRuntime::new(), root)
    }

    #[test]
    fn api_crud_round_trip() {
        let (_dir, runtime, root) = fixture();

        let created = api_create_thread(
            &runtime,
            CreateThreadParams {
                workspace_id: None,
                root: Some(root.clone()),
                id: Some("thread-api".into()),
                title: Some("API thread".into()),
            },
        )
        .expect("create");
        assert_eq!(created.thread.id, "thread-api");
        assert_eq!(created.thread.title.as_deref(), Some("API thread"));

        let listed = api_list_threads(
            &runtime,
            WorkspaceScopeParams {
                workspace_id: None,
                root: Some(root.clone()),
            },
        )
        .expect("list");
        assert_eq!(listed.threads.len(), 1);

        let appended = api_append_message(
            &runtime,
            "thread-api",
            AppendMessageParams {
                workspace_id: None,
                root: Some(root.clone()),
                id: Some("msg-api".into()),
                role: "user".into(),
                content: Some(json!({ "type": "text", "text": "hello agent" })),
                content_json: None,
                run_id: Some("run-1".into()),
            },
        )
        .expect("append");
        assert_eq!(appended.message.id, "msg-api");
        assert_eq!(
            appended.message.content["text"].as_str(),
            Some("hello agent")
        );

        let fetched = api_get_thread(
            &runtime,
            "thread-api",
            WorkspaceScopeParams {
                workspace_id: None,
                root: Some(root),
            },
        )
        .expect("get");
        assert_eq!(fetched.messages.len(), 1);
        assert_eq!(fetched.messages[0].role, "user");
    }
}
