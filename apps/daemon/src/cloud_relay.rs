//! Optional outbound Lattice Cloud device relay client.
//!
//! When `LATTICE_CLOUD_URL`, `LATTICE_CLOUD_TOKEN`, and `LATTICE_DEVICE_ID` are
//! set, latticed maintains a WebSocket to `GET /v1/devices/relay` and answers
//! [`lattice_mcp_catalog::RelayRequest`]s by dispatching local MCP tool calls.

use std::sync::Arc;
use std::time::Duration;

use axum::http::HeaderValue;
use futures_util::{SinkExt, StreamExt};
use lattice_mcp_catalog::{RelayError, RelayRequest, RelayResponse};
use lattice_runtime::LatticeRuntime;
use serde_json::json;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};

use crate::mcp::{self, JsonRpcRequest};

const DEVICE_ID_HEADER: &str = "x-lattice-device-id";

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
        let session_token = std::env::var("LATTICE_CLOUD_TOKEN").ok()?.trim().to_string();
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
pub fn spawn_cloud_relay(runtime: Arc<LatticeRuntime>, config: CloudRelayConfig) {
    tokio::spawn(async move {
        loop {
            match run_once(runtime.clone(), &config).await {
                Ok(()) => info!("cloud device relay disconnected; reconnecting"),
                Err(err) => warn!(error = %err, "cloud device relay error; retrying"),
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
}

async fn run_once(runtime: Arc<LatticeRuntime>, config: &CloudRelayConfig) -> Result<(), String> {
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

    while let Some(msg) = stream.next().await {
        let msg = msg.map_err(|e| format!("ws read: {e}"))?;
        let Message::Text(text) = msg else {
            continue;
        };
        let request: RelayRequest = match serde_json::from_str(&text) {
            Ok(req) => req,
            Err(err) => {
                warn!(error = %err, "invalid relay request");
                continue;
            }
        };
        let response = handle_relay_request(runtime.as_ref(), request);
        let payload =
            serde_json::to_string(&response).map_err(|e| format!("serialize response: {e}"))?;
        sink.send(Message::Text(payload.into()))
            .await
            .map_err(|e| format!("ws write: {e}"))?;
    }
    Ok(())
}

fn handle_relay_request(runtime: &LatticeRuntime, request: RelayRequest) -> RelayResponse {
    let mut arguments = request.arguments;
    if let Some(obj) = arguments.as_object_mut() {
        obj.entry("workspace_id")
            .or_insert_with(|| json!(request.workspace_id.clone()));
        obj.entry("workspaceId")
            .or_insert_with(|| json!(request.workspace_id.clone()));
    }

    let rpc = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(request.request_id.clone())),
        method: "tools/call".into(),
        params: json!({
            "name": request.tool_name,
            "arguments": arguments,
        }),
    };

    match mcp::dispatch(runtime, &rpc) {
        Some(resp) => {
            if let Some(err) = resp.get("error") {
                RelayResponse {
                    request_id: request.request_id,
                    result: None,
                    error: Some(RelayError {
                        code: "mcp_error".into(),
                        message: err.to_string(),
                    }),
                }
            } else {
                RelayResponse {
                    request_id: request.request_id,
                    result: resp.get("result").cloned(),
                    error: None,
                }
            }
        }
        None => RelayResponse {
            request_id: request.request_id,
            result: None,
            error: Some(RelayError {
                code: "no_response".into(),
                message: "local MCP dispatch returned no response".into(),
            }),
        },
    }
}
