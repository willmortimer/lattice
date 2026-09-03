//! Optional outbound Lattice Cloud device relay client (protocol v2).
//!
//! When `LATTICE_CLOUD_URL`, `LATTICE_CLOUD_TOKEN`, and `LATTICE_DEVICE_ID` are
//! set, latticed maintains a WebSocket to `GET /v1/devices/relay` and answers
//! [`lattice_relay_protocol::Invoke`] frames via [`crate::tool_executor`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::http::HeaderValue;
use futures_util::{SinkExt, StreamExt};
use lattice_relay_protocol::{
    Cancel, DeviceHello, Invoke, InvokeResult, Pong, RelayError, RelayFrame, Welcome,
    WorkspaceAuthority, RELAY_PROTOCOL_VERSION,
};
use lattice_runtime::LatticeRuntime;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::idle::ConnectionTracker;
use crate::tool_executor::{self, ToolCall, ToolError};
use crate::workspace_registry::{sync_remote_access_lease, WorkspaceRegistry};

const DEVICE_ID_HEADER: &str = "x-lattice-device-id";
const OUTBOUND_QUEUE_CAPACITY: usize = 64;
const MAX_CONCURRENT_INVOKES: usize = 4;
const BACKOFF_INITIAL: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

static CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Configuration for the optional cloud relay connector.
#[derive(Debug, Clone)]
pub struct CloudRelayConfig {
    pub cloud_url: String,
    pub session_token: String,
    pub device_id: String,
}

impl CloudRelayConfig {
    /// Load from `LATTICE_CLOUD_URL`, `LATTICE_CLOUD_TOKEN`, `LATTICE_DEVICE_ID`.
    pub fn from_env() -> Option<Self> {
        let cloud_url = std::env::var("LATTICE_CLOUD_URL").ok()?.trim().to_string();
        let session_token = std::env::var("LATTICE_CLOUD_TOKEN")
            .ok()?
            .trim()
            .to_string();
        let device_id = std::env::var("LATTICE_DEVICE_ID").ok()?.trim().to_string();
        if cloud_url.is_empty() || session_token.is_empty() || device_id.is_empty() {
            return None;
        }
        Some(Self {
            cloud_url,
            session_token,
            device_id,
        })
    }

    fn relay_ws_url(&self) -> String {
        let base = self.cloud_url.trim_end_matches('/');
        let http = if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else if base.starts_with("ws://") || base.starts_with("wss://") {
            base.to_string()
        } else {
            format!("wss://{base}")
        };
        format!("{http}/v1/devices/relay")
    }
}

/// Spawn a background reconnect loop. Returns immediately.
pub fn spawn_cloud_relay(
    runtime: Arc<LatticeRuntime>,
    config: CloudRelayConfig,
    connections: Option<Arc<ConnectionTracker>>,
) {
    tokio::spawn(async move {
        let mut backoff = BACKOFF_INITIAL;
        loop {
            match run_once(runtime.clone(), &config, connections.as_ref()).await {
                Ok(()) => {
                    info!("cloud device relay disconnected; reconnecting");
                    backoff = BACKOFF_INITIAL;
                }
                Err(err) => warn!(error = %err, "cloud device relay error; retrying"),
            }
            if let Some(tracker) = connections.as_ref() {
                sync_lease_from_registry(tracker).await;
            }
            tokio::time::sleep(backoff_with_jitter(backoff)).await;
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    });
}

fn next_connection_id() -> String {
    let generation = CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
    format!("conn-{generation}")
}

fn backoff_with_jitter(base: Duration) -> Duration {
    let base_ms = base.as_millis().max(1) as u64;
    let jitter_cap = (base_ms / 4).max(1);
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.subsec_nanos() as u64) % jitter_cap)
        .unwrap_or(0);
    base + Duration::from_millis(jitter)
}

fn workspaces_from_registry(registry: &WorkspaceRegistry) -> Vec<WorkspaceAuthority> {
    registry
        .list()
        .iter()
        .map(|entry| WorkspaceAuthority {
            workspace_id: entry.workspace_id.clone(),
            remote_access: entry.remote_access_enabled.then_some(true),
        })
        .collect()
}

fn should_sync_lease_on_connect(
    registry: &WorkspaceRegistry,
    workspaces: &[WorkspaceAuthority],
) -> bool {
    registry.remote_access_any() || workspaces.iter().any(|ws| ws.remote_access == Some(true))
}

async fn sync_lease_from_registry(tracker: &Arc<ConnectionTracker>) {
    if let Ok(registry) = WorkspaceRegistry::load_default() {
        sync_remote_access_lease(tracker, &registry).await;
    }
}

fn inject_workspace_id(arguments: &mut Value, workspace_id: &str) {
    if let Some(obj) = arguments.as_object_mut() {
        obj.entry("workspace_id")
            .or_insert_with(|| json!(workspace_id));
        obj.entry("workspaceId")
            .or_insert_with(|| json!(workspace_id));
    }
}

fn tool_error_to_relay(err: ToolError) -> RelayError {
    let code = match &err {
        ToolError::UnknownTool { .. } => "unknown_tool",
        ToolError::Execution { .. } => "tool_execution_failed",
    };
    RelayError {
        code: code.into(),
        message: err.to_string(),
    }
}

/// Map an inbound invoke to a relay result using the shared tool executor.
pub(crate) fn invoke_to_result(runtime: &LatticeRuntime, invoke: &Invoke) -> InvokeResult {
    let mut arguments = invoke.arguments.clone();
    inject_workspace_id(&mut arguments, &invoke.workspace_id);
    match tool_executor::execute(
        runtime,
        ToolCall {
            name: invoke.tool_name.clone(),
            arguments,
        },
    ) {
        Ok(result) => InvokeResult {
            request_id: invoke.request_id.clone(),
            result: Some(result),
            error: None,
        },
        Err(err) => InvokeResult {
            request_id: invoke.request_id.clone(),
            result: None,
            error: Some(tool_error_to_relay(err)),
        },
    }
}

fn invoke_timeout(deadline_ms: u64) -> Duration {
    if deadline_ms == 0 {
        Duration::from_secs(30)
    } else {
        Duration::from_millis(deadline_ms)
    }
}

fn parse_relay_frame(text: &str) -> Result<RelayFrame, String> {
    serde_json::from_str(text).map_err(|err| format!("invalid relay frame: {err}"))
}

fn check_welcome(welcome: &Welcome, expected_connection_id: &str) {
    if welcome.protocol_version != RELAY_PROTOCOL_VERSION {
        warn!(
            expected = RELAY_PROTOCOL_VERSION,
            actual = welcome.protocol_version,
            "relay welcome protocol_version mismatch"
        );
    }
    if welcome.connection_id != expected_connection_id {
        warn!(
            expected = %expected_connection_id,
            actual = %welcome.connection_id,
            "relay welcome connection_id mismatch"
        );
    }
}

async fn run_once(
    runtime: Arc<LatticeRuntime>,
    config: &CloudRelayConfig,
    connections: Option<&Arc<ConnectionTracker>>,
) -> Result<(), String> {
    let url = config.relay_ws_url();
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("relay url: {e}"))?;
    let headers = request.headers_mut();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", config.session_token))
            .map_err(|e| format!("authorization header: {e}"))?,
    );
    headers.insert(
        DEVICE_ID_HEADER,
        HeaderValue::from_str(&config.device_id).map_err(|e| format!("device header: {e}"))?,
    );

    let (ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    info!(%url, "cloud device relay connected");
    let (mut sink, mut stream) = ws.split();

    let registry = WorkspaceRegistry::load_default().unwrap_or_default();
    let workspaces = workspaces_from_registry(&registry);
    let connection_id = next_connection_id();
    let hello = RelayFrame::Hello(DeviceHello {
        protocol_version: RELAY_PROTOCOL_VERSION,
        device_id: config.device_id.clone(),
        connection_id: connection_id.clone(),
        workspaces: workspaces.clone(),
        catalog_hash: None,
    });
    let hello_payload =
        serde_json::to_string(&hello).map_err(|e| format!("serialize hello: {e}"))?;
    sink.send(Message::Text(hello_payload.into()))
        .await
        .map_err(|e| format!("ws write hello: {e}"))?;

    if let Some(tracker) = connections {
        if should_sync_lease_on_connect(&registry, &workspaces) {
            sync_remote_access_lease(tracker, &registry).await;
        }
    }

    let (outbound_tx, mut outbound_rx) = mpsc::channel::<RelayFrame>(OUTBOUND_QUEUE_CAPACITY);
    let invoke_semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_INVOKES));
    let cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>> =
        Arc::new(Mutex::new(HashMap::new()));

    loop {
        tokio::select! {
            msg = stream.next() => {
                let Some(msg) = msg else {
                    break;
                };
                let msg = msg.map_err(|e| format!("ws read: {e}"))?;
                let Message::Text(text) = msg else {
                    continue;
                };
                let frame = match parse_relay_frame(&text) {
                    Ok(frame) => frame,
                    Err(err) => {
                        warn!(error = %err, "invalid relay frame");
                        continue;
                    }
                };
                match frame {
                    RelayFrame::Welcome(welcome) => check_welcome(&welcome, &connection_id),
                    RelayFrame::Invoke(invoke) => {
                        let runtime = Arc::clone(&runtime);
                        let outbound_tx = outbound_tx.clone();
                        let cancel_tokens = Arc::clone(&cancel_tokens);
                        let semaphore = Arc::clone(&invoke_semaphore);
                        let request_id = invoke.request_id.clone();
                        let cancel_token = CancellationToken::new();
                        cancel_tokens
                            .lock()
                            .await
                            .insert(request_id.clone(), cancel_token.clone());
                        tokio::spawn(async move {
                            let Ok(permit) = semaphore.acquire_owned().await else {
                                return;
                            };
                            let deadline = invoke_timeout(invoke.deadline_ms);
                            let invoke_result = tokio::select! {
                                _ = cancel_token.cancelled() => InvokeResult {
                                    request_id: request_id.clone(),
                                    result: None,
                                    error: Some(RelayError {
                                        code: "cancelled".into(),
                                        message: "invoke cancelled".into(),
                                    }),
                                },
                                outcome = tokio::time::timeout(deadline, tokio::task::spawn_blocking({
                                    let runtime = Arc::clone(&runtime);
                                    let invoke = invoke.clone();
                                    move || invoke_to_result(runtime.as_ref(), &invoke)
                                })) => {
                                    match outcome {
                                        Ok(Ok(result)) => result,
                                        Ok(Err(join_err)) => InvokeResult {
                                            request_id: request_id.clone(),
                                            result: None,
                                            error: Some(RelayError {
                                                code: "internal_error".into(),
                                                message: format!("invoke task failed: {join_err}"),
                                            }),
                                        },
                                        Err(_) => InvokeResult {
                                            request_id: request_id.clone(),
                                            result: None,
                                            error: Some(RelayError {
                                                code: "deadline_exceeded".into(),
                                                message: "invoke timed out".into(),
                                            }),
                                        },
                                    }
                                }
                            };
                            cancel_tokens.lock().await.remove(&request_id);
                            drop(permit);
                            if outbound_tx
                                .send(RelayFrame::Result(invoke_result))
                                .await
                                .is_err()
                            {
                                warn!(%request_id, "relay outbound queue closed before result sent");
                            }
                        });
                    }
                    RelayFrame::Cancel(Cancel { request_id }) => {
                        if let Some(token) = cancel_tokens.lock().await.remove(&request_id) {
                            token.cancel();
                        }
                    }
                    RelayFrame::Ping(ping) => {
                        if outbound_tx
                            .send(RelayFrame::Pong(Pong { nonce: ping.nonce }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    RelayFrame::Result(_) | RelayFrame::Pong(_) | RelayFrame::Hello(_) => {
                        warn!("unexpected relay frame from gateway");
                    }
                }
            }
            outbound = outbound_rx.recv() => {
                let Some(frame) = outbound else {
                    break;
                };
                let payload = serde_json::to_string(&frame)
                    .map_err(|e| format!("serialize outbound frame: {e}"))?;
                sink.send(Message::Text(payload.into()))
                    .await
                    .map_err(|e| format!("ws write: {e}"))?;
            }
        }
    }

    if let Some(tracker) = connections {
        sync_lease_from_registry(tracker).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use lattice_mcp_catalog::TOOL_WORKSPACE_SEARCH;
    use lattice_relay_protocol::RelayFrame;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn cloud_relay_does_not_reference_mcp_dispatch() {
        let source = include_str!("cloud_relay.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let forbidden_dispatch = concat!("mcp", "::dispatch");
        let forbidden_import = concat!("use crate::", "mcp");
        assert!(
            !production.contains(forbidden_dispatch),
            "cloud_relay must not call mcp dispatch"
        );
        assert!(
            !production.contains(forbidden_import),
            "cloud_relay must not import mcp module"
        );
    }

    #[test]
    fn invoke_to_result_maps_success() {
        let registry_dir = TempDir::new().unwrap();
        let registry_path = registry_dir.path().join("workspace-registry.json");
        std::env::set_var(
            crate::workspace_registry::LATTICE_WORKSPACE_REGISTRY_PATH_ENV,
            &registry_path,
        );

        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Relay").unwrap();
        std::fs::write(dir.path().join("Page.md"), "# relay-invoke-token\n").unwrap();
        let runtime = LatticeRuntime::new();
        let session = runtime.open_workspace_session(dir.path()).unwrap();
        let workspace_id = session.workspace_id().to_string();
        crate::workspace_registry::register_workspace(&workspace_id, dir.path()).unwrap();
        let invoke = Invoke {
            request_id: "req-1".into(),
            workspace_id,
            tool_name: TOOL_WORKSPACE_SEARCH.into(),
            arguments: json!({
                "query": "relay-invoke-token",
                "mode": "fts"
            }),
            deadline_ms: 5_000,
            idempotency_key: None,
            cancel_token: None,
        };
        let result = invoke_to_result(&runtime, &invoke);
        assert_eq!(result.request_id, "req-1");
        assert!(
            result.error.is_none(),
            "unexpected error: {:?}",
            result.error
        );
        assert!(result.result.is_some());
        let text = result.result.unwrap().to_string();
        assert!(text.contains("relay-invoke-token") || text.contains("Page.md"));
    }

    #[test]
    fn invoke_to_result_injects_workspace_id() {
        let runtime = LatticeRuntime::new();
        let invoke = Invoke {
            request_id: "req-2".into(),
            workspace_id: "ws-inject".into(),
            tool_name: "workspace.nonexistent".into(),
            arguments: json!({}),
            deadline_ms: 1_000,
            idempotency_key: None,
            cancel_token: None,
        };
        let result = invoke_to_result(&runtime, &invoke);
        assert_eq!(result.request_id, "req-2");
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap().code, "unknown_tool");
    }

    #[test]
    fn hello_frame_round_trip_from_registry() {
        let registry = WorkspaceRegistry {
            version: 1,
            workspaces: vec![crate::workspace_registry::WorkspaceRegistryRecord {
                workspace_id: "ws-alpha".into(),
                root: std::path::PathBuf::from("/tmp/ws"),
                remote_access_enabled: true,
            }],
        };
        let workspaces = workspaces_from_registry(&registry);
        let frame = RelayFrame::Hello(DeviceHello {
            protocol_version: RELAY_PROTOCOL_VERSION,
            device_id: "device-1".into(),
            connection_id: "conn-1".into(),
            workspaces,
            catalog_hash: None,
        });
        let raw = serde_json::to_string(&frame).unwrap();
        let parsed: RelayFrame = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, frame);
    }

    #[test]
    fn ping_maps_to_pong_nonce() {
        let ping = lattice_relay_protocol::Ping {
            nonce: Some("nonce-42".into()),
        };
        let pong = Pong {
            nonce: ping.nonce.clone(),
        };
        assert_eq!(pong.nonce, Some("nonce-42".into()));
    }
}
