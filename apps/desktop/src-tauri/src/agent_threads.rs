//! Workspace-local agent thread persistence via the latticed HTTP API.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
const ENV_API_PORT: &str = "LATTICE_API_PORT";
const DEFAULT_API_PORT: u16 = 18787;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThreadEnsureArgs {
    pub workspace_root: String,
    pub thread_id: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThreadAppendMessageArgs {
    pub workspace_root: String,
    pub thread_id: String,
    pub role: String,
    pub content: Value,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadDto {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateThreadResponse {
    thread: ThreadDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppendMessageResponse {
    message: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThreadListArgs {
    pub workspace_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThreadGetArgs {
    pub workspace_root: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThreadSummary {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentThreadMessage {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content: Value,
    pub run_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentThreadsResult {
    pub workspace_id: String,
    pub threads: Vec<AgentThreadSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAgentThreadResult {
    pub workspace_id: String,
    pub thread: AgentThreadSummary,
    pub messages: Vec<AgentThreadMessage>,
}

fn http_base() -> Result<(String, u16), String> {
    let token = std::env::var(ENV_AUTH_TOKEN).map_err(|_| {
        "daemon auth token is unavailable; agent thread persistence requires latticed".into()
    })?;
    if token.trim().is_empty() {
        return Err("daemon auth token is empty".into());
    }
    let port = std::env::var(ENV_API_PORT)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_API_PORT);
    Ok((token, port))
}

fn percent_encode_query(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn http_get_json(path: &str) -> Result<(u16, Value), String> {
    let (token, port) = http_base()?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let response = ureq::get(&url)
        .set("authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(5))
        .call()
        .map_err(|err| format!("agent thread HTTP request failed: {err}"))?;
    let status = response.status();
    let text = response
        .into_string()
        .map_err(|err| format!("agent thread HTTP response body failed: {err}"))?;
    let value = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text)
            .map_err(|err| format!("agent thread HTTP JSON failed: {err}"))?
    };
    Ok((status, value))
}

fn http_post_json(path: &str, body: &Value) -> Result<Value, String> {
    let (token, port) = http_base()?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let response = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(5))
        .send_string(&body.to_string())
        .map_err(|err| format!("agent thread HTTP request failed: {err}"))?;
    let status = response.status();
    let text = response
        .into_string()
        .map_err(|err| format!("agent thread HTTP response body failed: {err}"))?;
    if status < 200 || status >= 300 {
        return Err(format!("agent thread HTTP {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|err| format!("agent thread HTTP JSON failed: {err}"))
}

/// Create a workspace-local agent thread when it does not already exist.
#[tauri::command]
pub fn agent_thread_ensure(args: AgentThreadEnsureArgs) -> Result<(), String> {
    if args.workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    if args.thread_id.trim().is_empty() {
        return Err("thread id is required".into());
    }

    let get_path = format!(
        "/v1/agent_threads/{}?root={}",
        args.thread_id,
        percent_encode_query(&args.workspace_root),
    );
    if let Ok((status, _)) = http_get_json(&get_path) {
        if status == 200 {
            return Ok(());
        }
    }

    let mut body = serde_json::json!({
        "root": args.workspace_root,
        "id": args.thread_id,
    });
    if let Some(title) = args.title.filter(|value| !value.trim().is_empty()) {
        body["title"] = Value::String(title);
    }

    let response: CreateThreadResponse = serde_json::from_value(
        http_post_json("/v1/agent_threads", &body)?,
    )
    .map_err(|err| format!("unexpected create thread response: {err}"))?;

    if response.thread.id != args.thread_id {
        return Err(format!(
            "create thread returned mismatched id: expected {}, got {}",
            args.thread_id, response.thread.id
        ));
    }
    Ok(())
}

/// Append one message to a workspace-local agent thread.
#[tauri::command]
pub fn agent_thread_append_message(args: AgentThreadAppendMessageArgs) -> Result<(), String> {
    if args.workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    if args.thread_id.trim().is_empty() {
        return Err("thread id is required".into());
    }
    if args.role.trim().is_empty() {
        return Err("role is required".into());
    }

    let mut body = serde_json::json!({
        "root": args.workspace_root,
        "role": args.role.trim(),
        "content": args.content,
    });
    if let Some(run_id) = args.run_id.filter(|value| !value.trim().is_empty()) {
        body["runId"] = Value::String(run_id);
    }
    if let Some(message_id) = args.message_id.filter(|value| !value.trim().is_empty()) {
        body["id"] = Value::String(message_id);
    }

    let path = format!("/v1/agent_threads/{}/messages", args.thread_id);
    let _: AppendMessageResponse = serde_json::from_value(http_post_json(&path, &body)?)
        .map_err(|err| format!("unexpected append message response: {err}"))?;
    Ok(())
}

/// List workspace-local agent threads (metadata only).
#[tauri::command]
pub fn agent_thread_list(args: AgentThreadListArgs) -> Result<ListAgentThreadsResult, String> {
    if args.workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    let path = format!(
        "/v1/agent_threads?root={}",
        percent_encode_query(&args.workspace_root),
    );
    let (status, value) = http_get_json(&path)?;
    if status < 200 || status >= 300 {
        return Err(format!("agent thread HTTP {status}: {value}"));
    }
    serde_json::from_value(value).map_err(|err| format!("unexpected list threads response: {err}"))
}

/// Fetch one workspace-local agent thread and its messages.
#[tauri::command]
pub fn agent_thread_get(args: AgentThreadGetArgs) -> Result<GetAgentThreadResult, String> {
    if args.workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    if args.thread_id.trim().is_empty() {
        return Err("thread id is required".into());
    }
    let path = format!(
        "/v1/agent_threads/{}?root={}",
        args.thread_id,
        percent_encode_query(&args.workspace_root),
    );
    let (status, value) = http_get_json(&path)?;
    if status == 404 {
        return Err(format!("thread not found: {}", args.thread_id));
    }
    if status < 200 || status >= 300 {
        return Err(format!("agent thread HTTP {status}: {value}"));
    }
    serde_json::from_value(value).map_err(|err| format!("unexpected get thread response: {err}"))
}
