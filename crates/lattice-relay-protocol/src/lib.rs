//! Versioned JSON frame types for the Lattice Relay Protocol (v2).
//!
//! This crate is intentionally inert: it has no I/O, no transport, and no
//! dispatcher. It only describes the wire shapes for device ↔ gateway relay
//! sessions (Hello/Welcome handshake, Invoke/Cancel/Result tool plane, and
//! Ping/Pong keepalive).
//!
//! v1 [`lattice_mcp_catalog::RelayRequest`] / [`RelayResponse`] remain in
//! `lattice-mcp-catalog` for backward compatibility with existing cloud paths.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Current relay protocol version carried in Hello/Welcome frames.
pub const RELAY_PROTOCOL_VERSION: u32 = 2;

/// Top-level relay frame envelope (internally tagged by `type`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayFrame {
    Hello(DeviceHello),
    Welcome(Welcome),
    Invoke(Invoke),
    Cancel(Cancel),
    Result(InvokeResult),
    Ping(Ping),
    Pong(Pong),
}

/// Device → gateway session open; advertises workspace authority and catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceHello {
    pub protocol_version: u32,
    pub device_id: String,
    pub connection_id: String,
    pub workspaces: Vec<WorkspaceAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_hash: Option<String>,
}

/// Workspace the device authorizes the gateway to invoke tools against.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceAuthority {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_access: Option<bool>,
}

/// Gateway → device handshake acknowledging the connection generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Welcome {
    pub connection_id: String,
    pub protocol_version: u32,
}

/// Gateway → device tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Invoke {
    pub request_id: String,
    pub workspace_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_token: Option<String>,
}

/// Gateway → device cancellation for an in-flight invoke.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cancel {
    pub request_id: String,
}

/// Device → gateway result for an [`Invoke`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvokeResult {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RelayError>,
}

/// Structured relay failure (distinct from MCP JSON-RPC errors).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayError {
    pub code: String,
    pub message: String,
}

/// Keepalive probe (either direction).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

/// Keepalive response (either direction).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pong {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn round_trip_frame(frame: &RelayFrame) {
        let raw = serde_json::to_string(frame).unwrap();
        let parsed: RelayFrame = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, *frame, "round-trip failed for: {raw}");
    }

    #[test]
    fn hello_frame_round_trip() {
        let frame = RelayFrame::Hello(DeviceHello {
            protocol_version: RELAY_PROTOCOL_VERSION,
            device_id: "device-abc".into(),
            connection_id: "conn-1".into(),
            workspaces: vec![
                WorkspaceAuthority {
                    workspace_id: "ws-alpha".into(),
                    remote_access: Some(true),
                },
                WorkspaceAuthority {
                    workspace_id: "ws-beta".into(),
                    remote_access: None,
                },
            ],
            catalog_hash: Some("sha256:deadbeef".into()),
        });
        round_trip_frame(&frame);

        let raw = serde_json::to_string(&frame).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["type"], "hello");
        assert_eq!(value["protocol_version"], RELAY_PROTOCOL_VERSION);
        assert_eq!(value["workspaces"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn welcome_frame_round_trip() {
        round_trip_frame(&RelayFrame::Welcome(Welcome {
            connection_id: "conn-1".into(),
            protocol_version: RELAY_PROTOCOL_VERSION,
        }));
    }

    #[test]
    fn invoke_frame_round_trip() {
        round_trip_frame(&RelayFrame::Invoke(Invoke {
            request_id: "req-42".into(),
            workspace_id: "ws-alpha".into(),
            tool_name: "workspace.read".into(),
            arguments: json!({ "path": "Notes.md" }),
            deadline_ms: 5_000,
            idempotency_key: Some("idem-1".into()),
            cancel_token: Some("cancel-1".into()),
        }));
    }

    #[test]
    fn cancel_frame_round_trip() {
        round_trip_frame(&RelayFrame::Cancel(Cancel {
            request_id: "req-42".into(),
        }));
    }

    #[test]
    fn result_frame_round_trip_success() {
        round_trip_frame(&RelayFrame::Result(InvokeResult {
            request_id: "req-42".into(),
            result: Some(json!({ "bytes": 128 })),
            error: None,
        }));
    }

    #[test]
    fn result_frame_round_trip_error() {
        round_trip_frame(&RelayFrame::Result(InvokeResult {
            request_id: "req-42".into(),
            result: None,
            error: Some(RelayError {
                code: "deadline_exceeded".into(),
                message: "invoke timed out".into(),
            }),
        }));
    }

    #[test]
    fn ping_frame_round_trip_with_nonce() {
        round_trip_frame(&RelayFrame::Ping(Ping {
            nonce: Some("nonce-9".into()),
        }));
    }

    #[test]
    fn ping_frame_round_trip_without_nonce() {
        round_trip_frame(&RelayFrame::Ping(Ping { nonce: None }));
    }

    #[test]
    fn pong_frame_round_trip_with_nonce() {
        round_trip_frame(&RelayFrame::Pong(Pong {
            nonce: Some("nonce-9".into()),
        }));
    }

    #[test]
    fn pong_frame_round_trip_without_nonce() {
        round_trip_frame(&RelayFrame::Pong(Pong { nonce: None }));
    }
}
