//! Thin Connect/JSON HTTP client for supervised local `celld` (`cell.v1` control plane).
//!
//! Mirrors `scripts/vz-lattice-loop.sh`: apply lattice-runtime spec, start the cell,
//! then invoke `lattice.runtime.v1` / `Ping` over the guest channel.

use std::net::SocketAddr;
use std::time::Duration;

use base64::Engine;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};

pub const DEFAULT_CELL_ID: &str = "cell_lattice_vz";
pub const LATTICE_RUNTIME_PROFILE: &str = "lattice-runtime";
pub const LATTICE_RUNTIME_MEMORY_BYTES: u64 = 4_294_967_296;

const APPLY_PATH: &str = "/cell.v1.CellService/ApplyCell";
const GET_CELL_PATH: &str = "/cell.v1.CellService/GetCell";
const START_CELL_PATH: &str = "/cell.v1.CellService/StartCell";
const INVOKE_PATH: &str = "/cell.v1.GuestSessionService/Invoke";

/// Errors from celld HTTP calls (honest strings for status surfaces).
#[derive(Debug)]
pub struct CelldClientError {
    pub message: String,
}

impl CelldClientError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CelldClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CelldClientError {}

/// Blocking HTTP client for a local `celld --http-dev` instance.
#[derive(Debug, Clone)]
pub struct CelldClient {
    base_url: String,
    cell_id: String,
    http: Client,
}

impl CelldClient {
    pub fn new(listen: SocketAddr, cell_id: impl Into<String>) -> Self {
        let cell_id = cell_id.into();
        let base_url = format!("http://{listen}");
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url,
            cell_id,
            http,
        }
    }

    pub fn healthz_ok(&self) -> bool {
        self.http
            .get(format!("{}/healthz", self.base_url))
            .send()
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    }

    pub fn apply_lattice_cell(&self) -> Result<(), CelldClientError> {
        let body = json!({
            "spec": lattice_cell_spec(&self.cell_id),
        });
        let _: Value = self.post_json(APPLY_PATH, body)?;
        Ok(())
    }

    pub fn get_observed_state(&self) -> Result<Option<String>, CelldClientError> {
        let body = json!({ "cellId": self.cell_id });
        let resp: Value = self.post_json(GET_CELL_PATH, body)?;
        Ok(resp
            .pointer("/cell/observedState")
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    pub fn start_cell(&self) -> Result<(), CelldClientError> {
        let body = json!({ "cellId": self.cell_id });
        let _: Value = self.post_json(START_CELL_PATH, body)?;
        Ok(())
    }

    pub fn invoke_lattice_ping(&self) -> Result<Value, CelldClientError> {
        let body = json!({
            "cellId": self.cell_id,
            "service": "lattice.runtime.v1",
            "method": "Ping",
            "contentType": "application/json",
        });
        let raw = self.post_stream(INVOKE_PATH, body)?;
        parse_ping_payload(&raw)
    }

    fn post_json(&self, path: &str, body: Value) -> Result<Value, CelldClientError> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|err| CelldClientError::new(format!("celld request failed: {err}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .map_err(|err| CelldClientError::new(format!("read celld response: {err}")))?;
        if !status.is_success() {
            return Err(CelldClientError::new(format!(
                "celld {path} returned {status}: {text}"
            )));
        }
        serde_json::from_str(&text)
            .map_err(|err| CelldClientError::new(format!("decode celld JSON: {err}; body={text}")))
    }

    fn post_stream(&self, path: &str, body: Value) -> Result<Vec<u8>, CelldClientError> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|err| CelldClientError::new(format!("celld invoke failed: {err}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .map_err(|err| CelldClientError::new(format!("read invoke response: {err}")))?;
        if !status.is_success() {
            return Err(CelldClientError::new(format!(
                "celld invoke returned {status}: {text}"
            )));
        }
        collect_invoke_payload(&text)
    }
}

fn lattice_cell_spec(cell_id: &str) -> Value {
    json!({
        "id": cell_id,
        "displayName": "Lattice VZ desktop",
        "profile": { "name": LATTICE_RUNTIME_PROFILE },
        "resources": {
            "vcpu": 2,
            "memoryBytes": LATTICE_RUNTIME_MEMORY_BYTES,
        },
        "volumes": [{
            "volumeId": "volume_lattice_data",
            "role": "data",
            "mount": "/mnt/cell/data",
            "mode": "ATTACHMENT_MODE_READ_WRITE",
            "required": true,
        }],
        "lifecycle": { "checkpointOnStop": true },
    })
}

#[derive(Debug, Deserialize)]
struct InvokeLine {
    #[serde(default)]
    payload: Option<String>,
    #[serde(default, rename = "errorMessage")]
    error_message: Option<String>,
    #[serde(default)]
    done: bool,
}

fn collect_invoke_payload(body: &str) -> Result<Vec<u8>, CelldClientError> {
    let mut merged = Vec::new();
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let frame: InvokeLine = serde_json::from_str(line).map_err(|err| {
            CelldClientError::new(format!("decode invoke frame: {err}; line={line}"))
        })?;
        if let Some(message) = frame.error_message.filter(|msg| !msg.is_empty()) {
            return Err(CelldClientError::new(message));
        }
        if let Some(payload_b64) = frame.payload.filter(|value| !value.is_empty()) {
            let chunk = base64::engine::general_purpose::STANDARD
                .decode(payload_b64.as_bytes())
                .map_err(|err| {
                    CelldClientError::new(format!("decode invoke payload base64: {err}"))
                })?;
            merged.extend_from_slice(&chunk);
        }
        if frame.done && !merged.is_empty() {
            return Ok(merged);
        }
    }
    if merged.is_empty() {
        return Err(CelldClientError::new(
            "invoke returned no payload (is the guest running?)",
        ));
    }
    Ok(merged)
}

fn parse_ping_payload(raw: &[u8]) -> Result<Value, CelldClientError> {
    let value: Value = serde_json::from_slice(raw)
        .map_err(|err| CelldClientError::new(format!("decode Ping JSON: {err}")))?;
    Ok(value)
}

/// Returns true when a Ping payload matches the lattice.runtime.v1 contract.
pub fn ping_payload_ok(value: &Value) -> bool {
    value.get("ok").and_then(Value::as_bool) == Some(true)
        && value
            .get("service")
            .and_then(Value::as_str)
            .is_some_and(|service| service.contains("lattice.runtime.v1"))
}

/// Guest observed states that mean the VM path is up or booting.
pub fn observed_state_up(state: &str) -> bool {
    matches!(
        state,
        "OBSERVED_STATE_STARTING"
            | "OBSERVED_STATE_READY"
            | "OBSERVED_STATE_RUNNING"
            | "OBSERVED_STATE_REQUESTED"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_payload_ok_requires_service_and_ok() {
        let ok = json!({
            "service": "lattice.runtime.v1",
            "method": "Ping",
            "ok": true,
            "cell_id": "cell_test",
        });
        assert!(ping_payload_ok(&ok));
        assert!(!ping_payload_ok(&json!({"ok": true})));
    }

    #[test]
    fn collect_invoke_payload_decodes_base64_json() {
        let payload = json!({
            "service": "lattice.runtime.v1",
            "method": "Ping",
            "ok": true,
        });
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
        let body = format!(r#"{{"payload":"{encoded}","contentType":"application/json","done":true}}"#);
        let raw = collect_invoke_payload(&body).expect("payload");
        let parsed: Value = serde_json::from_slice(&raw).expect("json");
        assert!(ping_payload_ok(&parsed));
    }

    #[test]
    fn collect_invoke_payload_reads_ndjson_stream() {
        let payload = json!({"service":"lattice.runtime.v1","ok":true});
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
        let body = format!(
            "{{\"payload\":\"{encoded}\"}}\n{{\"done\":true}}\n"
        );
        let raw = collect_invoke_payload(&body).expect("payload");
        let parsed: Value = serde_json::from_slice(&raw).expect("json");
        assert!(ping_payload_ok(&parsed));
    }

    #[test]
    fn lattice_cell_spec_matches_loop_profile() {
        let spec = lattice_cell_spec("cell_lattice_vz");
        assert_eq!(
            spec.pointer("/profile/name").and_then(Value::as_str),
            Some(LATTICE_RUNTIME_PROFILE)
        );
        assert_eq!(
            spec.pointer("/resources/memoryBytes").and_then(Value::as_u64),
            Some(LATTICE_RUNTIME_MEMORY_BYTES)
        );
    }

    #[test]
    fn parse_ping_payload_from_bytes() {
        let payload = br#"{"service":"lattice.runtime.v1","ok":true}"#;
        let value = parse_ping_payload(payload).expect("parse");
        assert!(ping_payload_ok(&value));
    }
}
