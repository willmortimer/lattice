//! Cell VZ lab env forwarding when the desktop spawns `latticed`.
//!
//! Supervision lives in `latticed`; this module only forwards parent env vars so
//! `pnpm tauri:dev` / `run-with-dotenv` can enable the lab path without manual
//! shell exports, and exposes `cell_status` for Settings polling.

use std::sync::Arc;

use lattice_client::{request, response, DaemonClient, LatticeClient, Request};
use lattice_protocol::GetCellStatusRequest;
use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::daemon_session::{self, SpawnHostEnv};

const ENV_CELL_VZ: &str = "LATTICE_CELL_VZ";
const ENV_CELL_HOST_BIN: &str = "LATTICE_CELL_HOST_BIN";
const ENV_CELLD_BIN: &str = "LATTICE_CELLD_BIN";
const ENV_CELL_VZ_IMAGES_DIR: &str = "CELL_VZ_IMAGES_DIR";
const ENV_CELL_VZ_HELPER_SOCKET: &str = "CELL_VZ_HELPER_SOCKET";
const ENV_CELL_DATA_DIR: &str = "LATTICE_CELL_DATA_DIR";
const ENV_CELL_LISTEN: &str = "LATTICE_CELL_LISTEN";

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

/// Append Cell VZ env vars when `LATTICE_CELL_VZ` is set in the parent process.
pub fn append_cell_vz_spawn_env(extra_env: &mut Vec<(String, String)>) {
    if !env_truthy(ENV_CELL_VZ) {
        return;
    }
    extra_env.push((ENV_CELL_VZ.to_string(), "1".into()));
    for key in [
        ENV_CELL_HOST_BIN,
        ENV_CELLD_BIN,
        ENV_CELL_VZ_IMAGES_DIR,
        ENV_CELL_VZ_HELPER_SOCKET,
        ENV_CELL_DATA_DIR,
        ENV_CELL_LISTEN,
    ] {
        forward_env_var(extra_env, key);
    }
}

#[derive(Default)]
pub struct CellState {
    inner: Mutex<CellInner>,
}

struct CellInner {
    client: Option<Arc<DaemonClient>>,
    _child: Option<daemon_session::SpawnedDaemon>,
}

impl Default for CellInner {
    fn default() -> Self {
        Self {
            client: None,
            _child: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellStatusDto {
    pub up: bool,
    pub ping_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

async fn ensure_daemon(inner: &mut CellInner) -> Result<Arc<DaemonClient>, String> {
    if let Some(client) = inner.client.as_ref() {
        return Ok(Arc::clone(client));
    }
    let mut host_env = SpawnHostEnv::default();
    append_cell_vz_spawn_env(&mut host_env.extra_env);
    let (client, child) = daemon_session::connect_or_spawn(host_env).await?;
    inner.client = Some(Arc::clone(&client));
    inner._child = child;
    Ok(client)
}

#[tauri::command]
pub async fn cell_status(state: State<'_, CellState>) -> Result<CellStatusDto, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_daemon(&mut inner).await?;
    drop(inner);

    let responded = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetCellStatus(GetCellStatusRequest {})),
        })
        .await
        .map_err(|err| format!("GetCellStatus failed: {err}"))?;

    match responded.body {
        Some(response::Body::GetCellStatus(resp)) => {
            let services = resp
                .services_json
                .as_deref()
                .and_then(|raw| serde_json::from_str(raw).ok());
            Ok(CellStatusDto {
                up: resp.up,
                ping_ok: resp.ping_ok,
                phase: resp.phase,
                services,
                error: resp.error,
            })
        }
        other => Err(format!("unexpected GetCellStatus response: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_noop_without_gate() {
        if std::env::var(ENV_CELL_VZ).is_ok() {
            return;
        }
        let mut env = Vec::new();
        append_cell_vz_spawn_env(&mut env);
        assert!(env.is_empty());
    }
}
