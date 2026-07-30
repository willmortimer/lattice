//! Golden JSON fixtures lock the Relay Protocol v2 wire shape across releases.

use lattice_relay_protocol::{
    Cancel, Invoke, InvokeResult, RelayError, RelayFrame, Welcome, RELAY_PROTOCOL_VERSION,
};
use serde_json::{json, Value};

fn fixture(name: &str) -> &'static str {
    match name {
        "hello" => include_str!("fixtures/hello.json"),
        "welcome" => include_str!("fixtures/welcome.json"),
        "invoke" => include_str!("fixtures/invoke.json"),
        "cancel" => include_str!("fixtures/cancel.json"),
        "result_success" => include_str!("fixtures/result_success.json"),
        "result_error" => include_str!("fixtures/result_error.json"),
        other => panic!("unknown fixture: {other}"),
    }
}

fn round_trip_frame(raw: &str) {
    let parsed: RelayFrame = serde_json::from_str(raw).unwrap();
    let reserialized = serde_json::to_string(&parsed).unwrap();
    let reparsed: RelayFrame = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(parsed, reparsed, "round-trip failed for fixture");
}

#[test]
fn golden_hello_fixture_wire_shape() {
    let raw = fixture("hello");
    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["type"], "hello");
    assert_eq!(value["protocol_version"], RELAY_PROTOCOL_VERSION);
    assert_eq!(value["device_id"], "device-golden");
    assert_eq!(value["connection_id"], "conn-golden-1");
    assert_eq!(value["catalog_hash"], "sha256:goldenfixture");
    let workspaces = value["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 2);
    assert_eq!(workspaces[0]["workspace_id"], "ws-alpha");
    assert_eq!(workspaces[0]["remote_access"], true);
    assert_eq!(workspaces[1]["workspace_id"], "ws-beta");
    assert!(workspaces[1].get("remote_access").is_none());

    round_trip_frame(raw);

    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    match frame {
        RelayFrame::Hello(hello) => {
            assert_eq!(hello.protocol_version, RELAY_PROTOCOL_VERSION);
            assert_eq!(hello.device_id, "device-golden");
            assert_eq!(hello.workspaces.len(), 2);
        }
        other => panic!("expected Hello, got {other:?}"),
    }
}

#[test]
fn golden_welcome_fixture_wire_shape() {
    let raw = fixture("welcome");
    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["type"], "welcome");
    assert_eq!(value["connection_id"], "conn-golden-1");
    assert_eq!(value["protocol_version"], RELAY_PROTOCOL_VERSION);
    round_trip_frame(raw);

    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    match frame {
        RelayFrame::Welcome(Welcome {
            connection_id,
            protocol_version,
        }) => {
            assert_eq!(connection_id, "conn-golden-1");
            assert_eq!(protocol_version, RELAY_PROTOCOL_VERSION);
        }
        other => panic!("expected Welcome, got {other:?}"),
    }
}

#[test]
fn golden_invoke_fixture_wire_shape() {
    let raw = fixture("invoke");
    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["type"], "invoke");
    assert_eq!(value["request_id"], "req-golden-42");
    assert_eq!(value["workspace_id"], "ws-alpha");
    assert_eq!(value["tool_name"], "workspace.read");
    assert_eq!(value["arguments"]["path"], "Notes.md");
    assert_eq!(value["deadline_ms"], 5000);
    assert_eq!(value["idempotency_key"], "idem-golden");
    assert_eq!(value["cancel_token"], "cancel-golden");
    round_trip_frame(raw);

    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    match frame {
        RelayFrame::Invoke(Invoke {
            request_id,
            workspace_id,
            tool_name,
            arguments,
            deadline_ms,
            idempotency_key,
            cancel_token,
        }) => {
            assert_eq!(request_id, "req-golden-42");
            assert_eq!(workspace_id, "ws-alpha");
            assert_eq!(tool_name, "workspace.read");
            assert_eq!(arguments, json!({ "path": "Notes.md" }));
            assert_eq!(deadline_ms, 5000);
            assert_eq!(idempotency_key, Some("idem-golden".into()));
            assert_eq!(cancel_token, Some("cancel-golden".into()));
        }
        other => panic!("expected Invoke, got {other:?}"),
    }
}

#[test]
fn golden_cancel_fixture_wire_shape() {
    let raw = fixture("cancel");
    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["type"], "cancel");
    assert_eq!(value["request_id"], "req-golden-42");
    round_trip_frame(raw);

    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    match frame {
        RelayFrame::Cancel(Cancel { request_id }) => assert_eq!(request_id, "req-golden-42"),
        other => panic!("expected Cancel, got {other:?}"),
    }
}

#[test]
fn golden_result_success_fixture_wire_shape() {
    let raw = fixture("result_success");
    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["type"], "result");
    assert_eq!(value["request_id"], "req-golden-42");
    assert_eq!(value["result"]["bytes"], 128);
    assert!(value.get("error").is_none());
    round_trip_frame(raw);

    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    match frame {
        RelayFrame::Result(InvokeResult {
            request_id,
            result,
            error,
        }) => {
            assert_eq!(request_id, "req-golden-42");
            assert_eq!(result, Some(json!({ "bytes": 128 })));
            assert!(error.is_none());
        }
        other => panic!("expected Result, got {other:?}"),
    }
}

#[test]
fn golden_result_error_fixture_wire_shape() {
    let raw = fixture("result_error");
    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["type"], "result");
    assert_eq!(value["request_id"], "req-golden-42");
    assert!(value.get("result").is_none());
    assert_eq!(value["error"]["code"], "deadline_exceeded");
    assert_eq!(value["error"]["message"], "invoke timed out");
    round_trip_frame(raw);

    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    match frame {
        RelayFrame::Result(InvokeResult {
            request_id,
            result,
            error,
        }) => {
            assert_eq!(request_id, "req-golden-42");
            assert!(result.is_none());
            assert_eq!(
                error,
                Some(RelayError {
                    code: "deadline_exceeded".into(),
                    message: "invoke timed out".into(),
                })
            );
        }
        other => panic!("expected Result, got {other:?}"),
    }
}

#[test]
fn golden_hello_matches_canonical_device_hello_type() {
    let raw = fixture("hello");
    let frame = serde_json::from_str::<RelayFrame>(raw).unwrap();
    let hello = match frame {
        RelayFrame::Hello(hello) => hello,
        other => panic!("expected Hello, got {other:?}"),
    };
    assert_eq!(hello.protocol_version, RELAY_PROTOCOL_VERSION);
    assert_eq!(hello.device_id, "device-golden");
    assert_eq!(hello.connection_id, "conn-golden-1");
    assert_eq!(hello.workspaces.len(), 2);
}
