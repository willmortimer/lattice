//! Workspace-local agent run-event log via the latticed HTTP API.
//!
//! Status + list-after-sequence power `reconnectToStream` replay. Live-tail is
//! implemented in [`crate::agent::agent_subscribe_run`] (bus wake + durable list).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
const ENV_API_PORT: &str = "LATTICE_API_PORT";
const DEFAULT_API_PORT: u16 = 18787;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunStatusArgs {
    pub workspace_root: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunListEventsArgs {
    pub workspace_root: String,
    pub run_id: String,
    #[serde(default)]
    pub after_sequence: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusDto {
    pub run_id: String,
    pub thread_id: String,
    pub status: String,
    pub last_sequence: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetRunStatusResult {
    pub workspace_id: String,
    pub run: Option<RunStatusDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListRunEventsResult {
    pub workspace_id: String,
    pub run_id: String,
    pub after_sequence: i64,
    pub events: Vec<RunEventDto>,
    pub run: RunStatusDto,
}

fn http_base() -> Result<(String, u16), String> {
    let token = std::env::var(ENV_AUTH_TOKEN).map_err(|_| {
        "daemon auth token is unavailable; agent run events require latticed".to_string()
    })?;
    if token.trim().is_empty() {
        return Err("daemon auth token is empty".to_string());
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
        .map_err(|err| format!("agent run-event HTTP request failed: {err}"))?;
    let status = response.status();
    let text = response
        .into_string()
        .map_err(|err| format!("agent run-event HTTP response body failed: {err}"))?;
    let value = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text)
            .map_err(|err| format!("agent run-event HTTP JSON failed: {err}"))?
    };
    Ok((status, value))
}

/// GET run status by run id or active run for a thread.
pub fn fetch_run_status(
    workspace_root: &str,
    run_id: Option<&str>,
    thread_id: Option<&str>,
) -> Result<GetRunStatusResult, String> {
    if workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    let root_q = percent_encode_query(workspace_root);
    let path = if let Some(id) = run_id.map(str::trim).filter(|id| !id.is_empty()) {
        format!("/v1/agent_runs/{id}?root={root_q}")
    } else if let Some(thread) = thread_id.map(str::trim).filter(|id| !id.is_empty()) {
        format!(
            "/v1/agent_runs?root={root_q}&threadId={}",
            percent_encode_query(thread)
        )
    } else {
        return Err("run id or thread id is required".into());
    };
    let (status, value) = http_get_json(&path)?;
    if status == 404 {
        return Ok(GetRunStatusResult {
            workspace_id: String::new(),
            run: None,
        });
    }
    if status < 200 || status >= 300 {
        return Err(format!("agent run-event HTTP {status}: {value}"));
    }
    serde_json::from_value(value).map_err(|err| format!("unexpected run status response: {err}"))
}

/// GET events with `event_sequence > after_sequence`.
pub fn fetch_run_events_after(
    workspace_root: &str,
    run_id: &str,
    after_sequence: i64,
) -> Result<ListRunEventsResult, String> {
    if workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    if run_id.trim().is_empty() {
        return Err("run id is required".into());
    }
    let after = after_sequence.max(0);
    let path = format!(
        "/v1/agent_runs/{}/events?root={}&afterSequence={}",
        run_id.trim(),
        percent_encode_query(workspace_root),
        after
    );
    let (status, value) = http_get_json(&path)?;
    if status == 404 {
        return Err(format!("run not found: {}", run_id.trim()));
    }
    if status < 200 || status >= 300 {
        return Err(format!("agent run-event HTTP {status}: {value}"));
    }
    serde_json::from_value(value).map_err(|err| format!("unexpected list events response: {err}"))
}

#[tauri::command]
pub fn agent_run_status(args: AgentRunStatusArgs) -> Result<GetRunStatusResult, String> {
    fetch_run_status(
        &args.workspace_root,
        args.run_id.as_deref(),
        args.thread_id.as_deref(),
    )
}

#[tauri::command]
pub fn agent_run_list_events(args: AgentRunListEventsArgs) -> Result<ListRunEventsResult, String> {
    fetch_run_events_after(
        &args.workspace_root,
        &args.run_id,
        args.after_sequence.unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::percent_encode_query;

    #[test]
    fn encodes_workspace_roots() {
        assert_eq!(percent_encode_query("/tmp/ws"), "/tmp/ws");
        assert_eq!(percent_encode_query("/tmp/my ws"), "/tmp/my%20ws");
    }
}
