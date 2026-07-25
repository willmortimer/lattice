//! Embedded agent thin-client for the desktop shell.
//!
//! Production path: Tauri → [`DaemonClient`] → `latticed` → Fake / agentd sidecar.
//! Streams ordered agent events over a Tauri [`Channel`] (not `app.emit`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lattice_client::{request, response, DaemonClient, EventFilter, LatticeClient, Request};
use lattice_protocol::{
    event, CancelAgentRunRequest, GetAgentHealthRequest, OpenWorkspaceRequest,
    StartAgentRunRequest,
};
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, State};
use tokio::sync::Mutex;

use crate::daemon_session::{self, SpawnHostEnv, SpawnedDaemon};

const ENV_AGENT_FAKE: &str = "LATTICE_AGENT_FAKE";
const ENV_AGENTD_BIN: &str = "LATTICE_AGENTD_BIN";
const ENV_AGENT_PROVIDER: &str = "LATTICE_AGENT_PROVIDER";
const ENV_AGENT_MODEL: &str = "LATTICE_AGENT_MODEL";
const ENV_PIONEER_API_KEY: &str = "PIONEER_API_KEY";
const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";

#[derive(Default)]
pub struct AgentState {
    inner: Mutex<AgentInner>,
}

struct AgentInner {
    client: Option<Arc<DaemonClient>>,
    _child: Option<SpawnedDaemon>,
    workspace_ids: HashMap<String, String>,
}

impl Default for AgentInner {
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
pub struct AgentHealthDto {
    pub ok: bool,
    pub backend: String,
    pub degraded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartRunResult {
    pub run_id: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentStreamMsg {
    UiChunk {
        run_id: String,
        chunk: serde_json::Value,
    },
    AgentEvent {
        run_id: String,
        event: serde_json::Value,
    },
    Done {
        run_id: String,
    },
    Error {
        run_id: String,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartRunArgs {
    pub workspace_root: String,
    pub thread_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub messages_json: Option<String>,
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn forward_env_var(extra_env: &mut Vec<(String, String)>, key: &str) {
    if let Ok(value) = std::env::var(key) {
        if !value.is_empty() {
            extra_env.push((key.to_string(), value));
        }
    }
}

fn discover_agentd_bin() -> Option<String> {
    if let Ok(path) = std::env::var(ENV_AGENTD_BIN) {
        if !path.is_empty() {
            return Some(path);
        }
    }
    // CARGO_MANIFEST_DIR is apps/desktop/src-tauri; canonicalize so latticed
    // sees apps/agentd/... instead of a src-tauri/../../agentd/... string.
    let run_sh = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../agentd/scripts/run.sh");
    let run_sh = std::fs::canonicalize(&run_sh).unwrap_or(run_sh);
    if run_sh.is_file() {
        return Some(run_sh.to_string_lossy().into());
    }
    None
}

/// Spawn env for latticed when the agent plane is first enabled from the desktop.
pub fn agent_spawn_env() -> SpawnHostEnv {
    let mut extra_env = Vec::new();

    let pioneer_key_set = std::env::var(ENV_PIONEER_API_KEY)
        .ok()
        .filter(|value| !value.is_empty())
        .is_some();

    let mut using_fake = false;
    if env_truthy(ENV_AGENT_FAKE) {
        extra_env.push((ENV_AGENT_FAKE.to_string(), "1".into()));
        using_fake = true;
    } else if !pioneer_key_set {
        extra_env.push((ENV_AGENT_FAKE.to_string(), "1".into()));
        using_fake = true;
    }

    if !using_fake {
        if let Some(bin) = discover_agentd_bin() {
            extra_env.push((ENV_AGENTD_BIN.to_string(), bin));
        }
    }

    for key in [
        ENV_PIONEER_API_KEY,
        ENV_OPENAI_API_KEY,
        ENV_AGENT_PROVIDER,
        ENV_AGENT_MODEL,
    ] {
        forward_env_var(&mut extra_env, key);
    }

    SpawnHostEnv {
        extra_env,
        handshake_hint: Some(
            "ensure agent runtime is available: set LATTICE_AGENT_FAKE=1 or LATTICE_AGENTD_BIN",
        ),
    }
}

async fn ensure_daemon(inner: &mut AgentInner) -> Result<Arc<DaemonClient>, String> {
    if let Some(client) = inner.client.as_ref() {
        return Ok(Arc::clone(client));
    }
    let (client, child) = daemon_session::connect_or_spawn(agent_spawn_env()).await?;
    inner.client = Some(Arc::clone(&client));
    inner._child = child;
    Ok(client)
}

async fn ensure_workspace(
    client: &DaemonClient,
    inner: &mut AgentInner,
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

fn parse_payload_json(payload_json: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(payload_json)
        .map_err(|err| format!("invalid agent event payload_json: {err}"))
}

/// Build channel messages for one daemon agent event.
///
/// Returns `(messages, terminal)` where `terminal` is true for run completion.
pub fn agent_event_messages(
    run_id: &str,
    event_type: &str,
    payload: &serde_json::Value,
) -> (Vec<AgentStreamMsg>, bool) {
    let mut messages = vec![AgentStreamMsg::AgentEvent {
        run_id: run_id.to_string(),
        event: payload.clone(),
    }];

    if event_type == "message_chunk" {
        if let Some(chunk) = payload.get("chunk") {
            messages.push(AgentStreamMsg::UiChunk {
                run_id: run_id.to_string(),
                chunk: chunk.clone(),
            });
        }
    }

    let terminal = match event_type {
        "run_completed" => {
            messages.push(AgentStreamMsg::Done {
                run_id: run_id.to_string(),
            });
            true
        }
        "run_failed" => {
            let message = payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("agent run failed")
                .to_string();
            messages.push(AgentStreamMsg::Error {
                run_id: run_id.to_string(),
                message,
            });
            true
        }
        _ => false,
    };

    (messages, terminal)
}

/// Forward one daemon agent event onto the Tauri channel.
///
/// Returns `true` when the run has reached a terminal state.
pub fn forward_agent_event(
    channel: &Channel<AgentStreamMsg>,
    run_id: &str,
    event_type: &str,
    payload: &serde_json::Value,
) -> bool {
    let (messages, terminal) = agent_event_messages(run_id, event_type, payload);
    for message in messages {
        let _ = channel.send(message);
    }
    terminal
}

#[tauri::command]
pub async fn agent_health(state: State<'_, AgentState>) -> Result<AgentHealthDto, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_daemon(&mut inner).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let responded = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetAgentHealth(GetAgentHealthRequest {})),
        })
        .await
        .map_err(|err| format!("GetAgentHealth failed: {err}"))?;

    match responded.body {
        Some(response::Body::GetAgentHealth(resp)) => {
            // Prefer an explicit provider kind. Older daemons may still report
            // transport `sidecar` — map that via LATTICE_AGENT_PROVIDER.
            let backend = match resp.backend.as_str() {
                "sidecar" => std::env::var(ENV_AGENT_PROVIDER)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .unwrap_or(resp.backend),
                other => other.to_string(),
            };
            Ok(AgentHealthDto {
                ok: resp.ok,
                backend,
                degraded: resp.degraded,
            })
        }
        other => Err(format!("unexpected GetAgentHealth response: {other:?}")),
    }
}

#[tauri::command]
pub async fn agent_start_run(
    args: AgentStartRunArgs,
    channel: Channel<AgentStreamMsg>,
    state: State<'_, AgentState>,
) -> Result<AgentStartRunResult, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_daemon(&mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &args.workspace_root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let mut events = client
        .subscribe(EventFilter {
            workspace_id: Some(workspace_id.clone()),
            agent_events_only: true,
        })
        .await
        .map_err(|err| format!("subscribe agent events failed: {err}"))?;

    let started = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::StartAgentRun(StartAgentRunRequest {
                workspace_id,
                thread_id: args.thread_id.clone(),
                run_id: args.run_id.clone(),
                provider: args.provider.unwrap_or_default(),
                model: args.model.unwrap_or_default(),
                prompt: args.prompt,
                messages_json: args.messages_json,
            })),
        })
        .await
        .map_err(|err| format!("StartAgentRun failed: {err}"))?;

    let (run_id, thread_id) = match started.body {
        Some(response::Body::StartAgentRun(resp)) => (resp.run_id, resp.thread_id),
        other => return Err(format!("unexpected StartAgentRun response: {other:?}")),
    };

    // Bound the wait so a dropped event stream cannot wedge the composer forever.
    let wait = async {
        while let Some(result) = events.next().await {
            let event = match result {
                Ok(event) => event,
                Err(err) => {
                    let _ = channel.send(AgentStreamMsg::Error {
                        run_id: run_id.clone(),
                        message: format!("agent event stream failed: {err}"),
                    });
                    return Ok(());
                }
            };
            let Some(event::Body::AgentEvent(agent_event)) = event.body else {
                continue;
            };
            if agent_event.run_id != run_id {
                continue;
            }
            let payload = parse_payload_json(&agent_event.payload_json)?;
            if forward_agent_event(&channel, &run_id, &agent_event.event_type, &payload) {
                return Ok(());
            }
        }
        let _ = channel.send(AgentStreamMsg::Error {
            run_id: run_id.clone(),
            message: "agent event stream ended before run completed".into(),
        });
        Ok::<(), String>(())
    };

    match tokio::time::timeout(std::time::Duration::from_secs(120), wait).await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err),
        Err(_) => {
            let _ = client
                .request(Request {
                    deadline_unix_ms: None,
                    idempotency_key: None,
                    body: Some(request::Body::CancelAgentRun(CancelAgentRunRequest {
                        run_id: run_id.clone(),
                    })),
                })
                .await;
            let _ = channel.send(AgentStreamMsg::Error {
                run_id: run_id.clone(),
                message: "agent run timed out waiting for events".into(),
            });
        }
    }

    Ok(AgentStartRunResult { run_id, thread_id })
}

#[tauri::command]
pub async fn agent_cancel_run(run_id: String, state: State<'_, AgentState>) -> Result<(), String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_daemon(&mut inner).await?;
    let client = Arc::clone(&client);
    drop(inner);

    client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::CancelAgentRun(CancelAgentRunRequest {
                run_id,
            })),
        })
        .await
        .map_err(|err| format!("CancelAgentRun failed: {err}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_spawn_env_defaults_fake_without_pioneer_key() {
        let _guard = EnvGuard::set(ENV_PIONEER_API_KEY, None);
        let _fake_guard = EnvGuard::set(ENV_AGENT_FAKE, None);
        let env = agent_spawn_env();
        assert!(
            env.extra_env
                .iter()
                .any(|(key, value)| key == ENV_AGENT_FAKE && value == "1"),
            "expected fake backend when Pioneer key is absent"
        );
    }

    #[test]
    fn agent_event_messages_maps_message_chunk() {
        let payload = serde_json::json!({
            "type": "message_chunk",
            "runId": "run-1",
            "chunk": { "type": "text-delta", "id": "c1", "delta": "hi" }
        });
        let (messages, terminal) =
            agent_event_messages("run-1", "message_chunk", &payload);
        assert!(!terminal);
        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[0], AgentStreamMsg::AgentEvent { .. }));
        match &messages[1] {
            AgentStreamMsg::UiChunk { chunk, .. } => assert_eq!(chunk["delta"], "hi"),
            other => panic!("expected ui chunk, got {other:?}"),
        }
    }

    #[test]
    fn agent_event_messages_maps_run_failed() {
        let payload = serde_json::json!({
            "type": "run_failed",
            "runId": "run-2",
            "message": "boom",
            "retryable": false
        });
        let (messages, terminal) = agent_event_messages("run-2", "run_failed", &payload);
        assert!(terminal);
        match messages.last() {
            Some(AgentStreamMsg::Error { message, .. }) => assert_eq!(message, "boom"),
            other => panic!("expected terminal error, got {other:?}"),
        }
    }

    struct EnvGuard {
        key: String,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &str, value: Option<&str>) -> Self {
            let previous = std::env::var(key).ok();
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
            Self {
                key: key.to_string(),
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(&self.key, value),
                None => std::env::remove_var(&self.key),
            }
        }
    }
}
