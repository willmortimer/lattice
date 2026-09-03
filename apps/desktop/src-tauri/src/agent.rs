//! Embedded agent thin-client for the desktop shell.
//!
//! Production path: Tauri → [`DaemonClient`] → `latticed` → Fake / agentd sidecar.
//! Streams ordered agent events over a Tauri [`Channel`] (not `app.emit`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lattice_client::{request, response, DaemonClient, EventFilter, LatticeClient, Request};
use lattice_protocol::{
    event, CancelAgentRunRequest, GetAgentHealthRequest, OpenWorkspaceRequest, StartAgentRunRequest,
};
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, State};
use tokio::sync::Mutex;

use crate::daemon_session::{self, SpawnHostEnv, SpawnedDaemon};

const ENV_AGENT_FAKE: &str = "LATTICE_AGENT_FAKE";
const ENV_AGENTD_BIN: &str = "LATTICE_AGENTD_BIN";
const ENV_AGENT_PROVIDER: &str = "LATTICE_AGENT_PROVIDER";
const ENV_AGENT_MODEL: &str = "LATTICE_AGENT_MODEL";
const ENV_LOCAL_LLM_BASE_URL: &str = "LATTICE_LOCAL_LLM_BASE_URL";
const ENV_LOCAL_LLM_API_KEY: &str = "LATTICE_LOCAL_LLM_API_KEY";
const ENV_LOCAL_LLM_MODEL: &str = "LATTICE_LOCAL_LLM_MODEL";
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
    /// Effective default model from `LATTICE_AGENT_MODEL` (empty when unset).
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartRunResult {
    pub run_id: String,
    pub thread_id: String,
    /// Set when the run ended in failure. Authoritative for the webview:
    /// Tauri Channel delivery can race the invoke promise, so the transport
    /// must not treat invoke resolution alone as success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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

    // Prefer the Rust sidecar (release, then debug, then next to this exe for packaged apps).
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for candidate in [
        workspace_root.join("target/release/lattice-agentd"),
        workspace_root.join("target/debug/lattice-agentd"),
    ] {
        let candidate = std::fs::canonicalize(&candidate).unwrap_or(candidate);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into());
        }
    }
    if let Some(sidecar) = crate::daemon_session::current_exe_sibling("lattice-agentd") {
        return Some(sidecar.to_string_lossy().into());
    }
    None
}

/// Spawn env for latticed when the agent plane is first enabled from the desktop.
pub fn agent_spawn_env() -> SpawnHostEnv {
    let mut extra_env = Vec::new();
    let desktop_ai = crate::ai::load_desktop_ai_settings();

    let account_credentials = crate::ai::resolve_account_ai_for_spawn();
    let account_ai_active = account_credentials.is_some();

    let pioneer_key_set = std::env::var(ENV_PIONEER_API_KEY)
        .ok()
        .filter(|value| !value.is_empty())
        .is_some();
    let resolved_openai_key = if account_ai_active {
        None
    } else {
        crate::ai::resolve_openai_api_key_for_spawn()
    };
    let openai_key_set = account_ai_active
        || resolved_openai_key.is_some()
        || std::env::var(ENV_OPENAI_API_KEY)
            .ok()
            .filter(|value| !value.is_empty())
            .is_some();

    let using_fake = crate::ai::should_use_fake_agent_backend(
        &desktop_ai,
        env_truthy(ENV_AGENT_FAKE),
        pioneer_key_set,
        openai_key_set,
    );
    if using_fake {
        extra_env.push((ENV_AGENT_FAKE.to_string(), "1".into()));
    }

    if !using_fake {
        if let Some(bin) = discover_agentd_bin() {
            extra_env.push((ENV_AGENTD_BIN.to_string(), bin));
        }
    }

    if let Some(credentials) = account_credentials {
        extra_env.extend(crate::ai::account_ai_spawn_env(&credentials));
    } else if let Some(key) = resolved_openai_key {
        extra_env.push((ENV_OPENAI_API_KEY.to_string(), key));
    }

    if account_ai_active {
        // Provider + OPENAI_* already set by account_ai_spawn_env.
        for key in [
            ENV_PIONEER_API_KEY,
            ENV_AGENT_MODEL,
            ENV_LOCAL_LLM_BASE_URL,
            ENV_LOCAL_LLM_API_KEY,
            ENV_LOCAL_LLM_MODEL,
        ] {
            forward_env_var(&mut extra_env, key);
        }
    } else {
        if let Some(provider) = crate::ai::agent_provider_for_profile(&desktop_ai) {
            extra_env.push((ENV_AGENT_PROVIDER.to_string(), provider.into()));
        } else {
            forward_env_var(&mut extra_env, ENV_AGENT_PROVIDER);
        }
        for key in [
            ENV_PIONEER_API_KEY,
            ENV_AGENT_MODEL,
            ENV_LOCAL_LLM_BASE_URL,
            ENV_LOCAL_LLM_API_KEY,
            ENV_LOCAL_LLM_MODEL,
        ] {
            forward_env_var(&mut extra_env, key);
        }
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
        "run_completed" | "run_cancelled" => {
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
                model: std::env::var(ENV_AGENT_MODEL)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_default(),
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
    // `Ok(None)` = completed; `Ok(Some(message))` = failed with message.
    let wait = async {
        while let Some(result) = events.next().await {
            let event = match result {
                Ok(event) => event,
                Err(err) => {
                    let message = format!("agent event stream failed: {err}");
                    let _ = channel.send(AgentStreamMsg::Error {
                        run_id: run_id.clone(),
                        message: message.clone(),
                    });
                    return Ok(Some(message));
                }
            };
            let Some(event::Body::AgentEvent(agent_event)) = event.body else {
                continue;
            };
            if agent_event.run_id != run_id {
                continue;
            }
            let payload = parse_payload_json(&agent_event.payload_json)?;
            let event_type = agent_event.event_type.as_str();
            if forward_agent_event(&channel, &run_id, event_type, &payload) {
                let error = if event_type == "run_failed" {
                    Some(
                        payload
                            .get("message")
                            .and_then(|value| value.as_str())
                            .unwrap_or("agent run failed")
                            .to_string(),
                    )
                } else {
                    None
                };
                return Ok(error);
            }
        }
        let message = "agent event stream ended before run completed".to_string();
        let _ = channel.send(AgentStreamMsg::Error {
            run_id: run_id.clone(),
            message: message.clone(),
        });
        Ok::<Option<String>, String>(Some(message))
    };

    let error = match tokio::time::timeout(std::time::Duration::from_secs(120), wait).await {
        Ok(Ok(error)) => error,
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
            let message = "agent run timed out waiting for events".to_string();
            let _ = channel.send(AgentStreamMsg::Error {
                run_id: run_id.clone(),
                message: message.clone(),
            });
            Some(message)
        }
    };

    Ok(AgentStartRunResult {
        run_id,
        thread_id,
        error,
    })
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubscribeRunArgs {
    pub workspace_root: String,
    pub run_id: String,
    #[serde(default)]
    pub after_sequence: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubscribeRunResult {
    pub run_id: String,
    pub thread_id: String,
    pub last_sequence: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_terminal_run_status(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled")
}

async fn list_run_events_blocking(
    workspace_root: String,
    run_id: String,
    after_sequence: i64,
) -> Result<crate::agent_run_events::ListRunEventsResult, String> {
    tokio::task::spawn_blocking(move || {
        crate::agent_run_events::fetch_run_events_after(&workspace_root, &run_id, after_sequence)
    })
    .await
    .map_err(|err| format!("list events join failed: {err}"))?
}

fn forward_run_event_rows(
    channel: &Channel<AgentStreamMsg>,
    events: &[crate::agent_run_events::RunEventDto],
    after_sequence: &mut i64,
) -> (bool, Option<String>) {
    for event in events {
        if event.event_sequence <= *after_sequence {
            continue;
        }
        *after_sequence = event.event_sequence;
        let terminal =
            forward_agent_event(channel, &event.run_id, &event.event_type, &event.payload);
        if terminal {
            let error = if event.event_type == "run_failed" {
                Some(
                    event
                        .payload
                        .get("message")
                        .and_then(|value| value.as_str())
                        .unwrap_or("agent run failed")
                        .to_string(),
                )
            } else {
                None
            };
            return (true, error);
        }
    }
    (false, None)
}

/// Replay durable events after `after_sequence`, then live-tail until terminal.
///
/// Gap-free handoff: wake on the daemon agent event bus, then always drain the
/// authoritative SQLite log via HTTP list-after-sequence (never trust bus-only).
#[tauri::command]
pub async fn agent_subscribe_run(
    args: AgentSubscribeRunArgs,
    channel: Channel<AgentStreamMsg>,
    state: State<'_, AgentState>,
) -> Result<AgentSubscribeRunResult, String> {
    if args.workspace_root.trim().is_empty() {
        return Err("workspace root is required".into());
    }
    if args.run_id.trim().is_empty() {
        return Err("run id is required".into());
    }
    let run_id = args.run_id.trim().to_string();
    let workspace_root = args.workspace_root.clone();
    let mut after_sequence = args.after_sequence.unwrap_or(0).max(0);

    let mut inner = state.inner.lock().await;
    let client = ensure_daemon(&mut inner).await?;
    let workspace_id = ensure_workspace(client.as_ref(), &mut inner, &workspace_root).await?;
    let client = Arc::clone(&client);
    drop(inner);

    let status = {
        let root = workspace_root.clone();
        let id = run_id.clone();
        tokio::task::spawn_blocking(move || {
            crate::agent_run_events::fetch_run_status(&root, Some(&id), None)
        })
        .await
        .map_err(|err| format!("run status join failed: {err}"))??
    };
    if status.run.is_none() {
        return Err(format!("run not found: {run_id}"));
    }

    let mut events = client
        .subscribe(EventFilter {
            workspace_id: Some(workspace_id),
            agent_events_only: true,
        })
        .await
        .map_err(|err| format!("subscribe agent events failed: {err}"))?;

    // Initial replay from the durable log.
    let replay =
        list_run_events_blocking(workspace_root.clone(), run_id.clone(), after_sequence).await?;
    let mut run = replay.run;
    let (terminal, mut error) =
        forward_run_event_rows(&channel, &replay.events, &mut after_sequence);
    if terminal {
        return Ok(AgentSubscribeRunResult {
            run_id,
            thread_id: run.thread_id,
            last_sequence: after_sequence,
            status: run.status,
            error,
        });
    }
    if is_terminal_run_status(&run.status) {
        let catchup =
            list_run_events_blocking(workspace_root.clone(), run_id.clone(), after_sequence)
                .await?;
        run = catchup.run;
        let (terminal, err) =
            forward_run_event_rows(&channel, &catchup.events, &mut after_sequence);
        error = err;
        if terminal || is_terminal_run_status(&run.status) {
            if !terminal {
                let _ = channel.send(AgentStreamMsg::Done {
                    run_id: run_id.clone(),
                });
            }
            return Ok(AgentSubscribeRunResult {
                run_id,
                thread_id: run.thread_id,
                last_sequence: after_sequence,
                status: run.status,
                error,
            });
        }
    }

    // Live-tail: bus wake (or poll interval) then drain the durable log.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        if tokio::time::Instant::now() >= deadline {
            let message = "agent subscribe timed out waiting for events".to_string();
            let _ = channel.send(AgentStreamMsg::Error {
                run_id: run_id.clone(),
                message: message.clone(),
            });
            return Ok(AgentSubscribeRunResult {
                run_id,
                thread_id: run.thread_id,
                last_sequence: after_sequence,
                status: "failed".into(),
                error: Some(message),
            });
        }

        tokio::select! {
            maybe = events.next() => {
                match maybe {
                    Some(Ok(event)) => {
                        if let Some(event::Body::AgentEvent(agent_event)) = event.body {
                            if agent_event.run_id != run_id {
                                continue;
                            }
                        }
                    }
                    Some(Err(err)) => {
                        let message = format!("agent event stream failed: {err}");
                        let _ = channel.send(AgentStreamMsg::Error {
                            run_id: run_id.clone(),
                            message: message.clone(),
                        });
                        return Ok(AgentSubscribeRunResult {
                            run_id,
                            thread_id: run.thread_id,
                            last_sequence: after_sequence,
                            status: "failed".into(),
                            error: Some(message),
                        });
                    }
                    None => {}
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
        }

        let listed =
            list_run_events_blocking(workspace_root.clone(), run_id.clone(), after_sequence)
                .await?;
        run = listed.run;
        let (terminal, err) = forward_run_event_rows(&channel, &listed.events, &mut after_sequence);
        if terminal {
            return Ok(AgentSubscribeRunResult {
                run_id,
                thread_id: run.thread_id,
                last_sequence: after_sequence,
                status: run.status,
                error: err,
            });
        }
        if is_terminal_run_status(&run.status) && listed.events.is_empty() {
            let _ = channel.send(AgentStreamMsg::Done {
                run_id: run_id.clone(),
            });
            return Ok(AgentSubscribeRunResult {
                run_id,
                thread_id: run.thread_id,
                last_sequence: after_sequence,
                status: run.status,
                error: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn agent_spawn_env_defaults_fake_without_provider_keys() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = EnvGuard::set(ENV_PIONEER_API_KEY, None);
        let _openai = EnvGuard::set(ENV_OPENAI_API_KEY, None);
        let _fake_guard = EnvGuard::set(ENV_AGENT_FAKE, None);
        let _bin = EnvGuard::set(ENV_AGENTD_BIN, None);
        let env = agent_spawn_env();
        assert!(
            env.extra_env
                .iter()
                .any(|(key, value)| key == ENV_AGENT_FAKE && value == "1"),
            "expected fake backend when provider keys are absent"
        );
    }

    #[test]
    fn agent_spawn_env_forces_openai_provider_for_byo_profile() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = EnvGuard::set(ENV_PIONEER_API_KEY, Some("pioneer-test"));
        let _openai = EnvGuard::set(ENV_OPENAI_API_KEY, None);
        let _fake_guard = EnvGuard::set(ENV_AGENT_FAKE, None);
        let _bin = EnvGuard::set(ENV_AGENTD_BIN, Some("/tmp/lattice-agentd-test"));
        let _provider = EnvGuard::set(ENV_AGENT_PROVIDER, None);

        let mut settings = crate::ai::load_desktop_ai_settings();
        settings.mode = lattice_profile::AiMode::ByoOpenai;
        let provider = crate::ai::agent_provider_for_profile(&settings);
        assert_eq!(provider, Some("openai"));
        assert!(!crate::ai::should_use_fake_agent_backend(
            &settings, false, true, false
        ));
    }

    #[test]
    fn agent_spawn_env_uses_sidecar_when_openai_key_present() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = EnvGuard::set(ENV_PIONEER_API_KEY, None);
        let _openai = EnvGuard::set(ENV_OPENAI_API_KEY, Some("sk-test"));
        let _fake_guard = EnvGuard::set(ENV_AGENT_FAKE, None);
        let _bin = EnvGuard::set(ENV_AGENTD_BIN, Some("/tmp/lattice-agentd-test"));
        let env = agent_spawn_env();
        assert!(
            !env.extra_env
                .iter()
                .any(|(key, value)| key == ENV_AGENT_FAKE && value == "1"),
            "openai key should not force fake"
        );
        assert!(
            env.extra_env
                .iter()
                .any(|(key, value)| key == ENV_AGENTD_BIN && value == "/tmp/lattice-agentd-test"),
            "explicit LATTICE_AGENTD_BIN should be forwarded"
        );
    }

    #[test]
    fn agent_event_messages_maps_message_chunk() {
        let payload = serde_json::json!({
            "type": "message_chunk",
            "runId": "run-1",
            "chunk": { "type": "text-delta", "id": "c1", "delta": "hi" }
        });
        let (messages, terminal) = agent_event_messages("run-1", "message_chunk", &payload);
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

    #[test]
    fn agent_event_messages_maps_run_cancelled() {
        let payload = serde_json::json!({
            "type": "run_cancelled",
            "runId": "run-3"
        });
        let (messages, terminal) = agent_event_messages("run-3", "run_cancelled", &payload);
        assert!(terminal);
        assert!(matches!(messages.last(), Some(AgentStreamMsg::Done { .. })));
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
