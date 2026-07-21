//! Tauri-side wiring for out-of-process ipykernel sessions (Phase-4 J2).
//!
//! Keeps `lattice-kernel` free of Tauri: this module owns session state and
//! workspace capability gating.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lattice_core::Workspace;
use lattice_kernel::{ExecuteResult, KernelSessionMap, StartOptions};
use serde::{Deserialize, Serialize};

/// Tauri-managed map of live kernel bridge sessions.
#[derive(Default)]
pub struct KernelState(Mutex<KernelSessionMap>);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelStartRequest {
    /// Workspace root (capability + cwd gate).
    pub root: String,
    /// Optional cwd relative to or under `root`; defaults to `root`.
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelStartResponse {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelExecuteRequest {
    pub session_id: String,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelSessionRequest {
    pub session_id: String,
}

/// Open `root` and require `"jupyter"` in `lattice.yaml` capabilities.enabled.
fn require_jupyter_capability(root: &Path) -> Result<Workspace, String> {
    let workspace = Workspace::open(root).map_err(|err| err.to_string())?;
    if !workspace
        .manifest()
        .capabilities
        .enabled
        .iter()
        .any(|capability| capability == "jupyter")
    {
        return Err("jupyter capability is not enabled".into());
    }
    Ok(workspace)
}

fn unknown_session(session_id: &str) -> String {
    format!("unknown kernel session: {session_id}")
}

/// Start an out-of-process ipykernel bridge under the workspace root.
#[tauri::command]
pub fn kernel_start(
    request: KernelStartRequest,
    state: tauri::State<'_, KernelState>,
) -> Result<KernelStartResponse, String> {
    let workspace = require_jupyter_capability(Path::new(&request.root))?;
    let cwd = request
        .cwd
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.root().to_path_buf());

    let mut map = state.0.lock().map_err(|_| "kernel state lock poisoned")?;
    let session_id = map
        .start(StartOptions::new(workspace.root(), cwd))
        .map_err(|err| err.to_string())?;
    Ok(KernelStartResponse { session_id })
}

/// Execute code in a live session; returns collected Jupyter-shaped outputs.
#[tauri::command]
pub fn kernel_execute(
    request: KernelExecuteRequest,
    state: tauri::State<'_, KernelState>,
) -> Result<ExecuteResult, String> {
    // Clone the handle under a short lock so interrupt can run concurrently.
    let session = {
        let map = state.0.lock().map_err(|_| "kernel state lock poisoned")?;
        map.get(&request.session_id)
            .map_err(|_| unknown_session(&request.session_id))?
    };
    session
        .execute(request.code)
        .map_err(|err| err.to_string())
}

/// Interrupt in-flight execution on a live session.
#[tauri::command]
pub fn kernel_interrupt(
    request: KernelSessionRequest,
    state: tauri::State<'_, KernelState>,
) -> Result<(), String> {
    let session = {
        let map = state.0.lock().map_err(|_| "kernel state lock poisoned")?;
        map.get(&request.session_id)
            .map_err(|_| unknown_session(&request.session_id))?
    };
    session.interrupt().map_err(|err| err.to_string())
}

/// Shut down and remove a session (kill-on-drop if the bridge is unresponsive).
#[tauri::command]
pub fn kernel_shutdown(
    request: KernelSessionRequest,
    state: tauri::State<'_, KernelState>,
) -> Result<(), String> {
    let mut map = state.0.lock().map_err(|_| "kernel state lock poisoned")?;
    map.shutdown(&request.session_id)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::{Workspace, WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME};

    fn init_workspace_without_jupyter() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        Workspace::init(dir.path(), "Kernel Test").expect("init workspace");
        dir
    }

    fn enable_jupyter(root: &Path) {
        let manifest_path = root.join(WORKSPACE_MANIFEST_FILENAME);
        let mut manifest = WorkspaceManifest::load(&manifest_path).expect("load manifest");
        if !manifest
            .capabilities
            .enabled
            .iter()
            .any(|capability| capability == "jupyter")
        {
            manifest.capabilities.enabled.push("jupyter".into());
        }
        manifest.save(&manifest_path).expect("save manifest");
    }

    #[test]
    fn start_denies_when_jupyter_capability_missing() {
        let dir = init_workspace_without_jupyter();
        let err = require_jupyter_capability(dir.path()).expect_err("deny");
        assert_eq!(err, "jupyter capability is not enabled");
    }

    #[test]
    fn start_gate_accepts_enabled_jupyter_capability() {
        let dir = init_workspace_without_jupyter();
        enable_jupyter(dir.path());
        let workspace = require_jupyter_capability(dir.path()).expect("allow");
        assert_eq!(workspace.root(), dir.path());
    }

    #[test]
    fn start_response_serializes_camel_case_session_id() {
        let json = serde_json::to_value(KernelStartResponse {
            session_id: "kernel-1".into(),
        })
        .expect("serialize");
        assert_eq!(json["sessionId"], "kernel-1");
        assert!(json.get("session_id").is_none());
    }

    #[test]
    fn execute_interrupt_shutdown_reject_unknown_session_id() {
        let state = KernelState::default();
        let missing = KernelExecuteRequest {
            session_id: "kernel-missing".into(),
            code: "1".into(),
        };
        let map = state.0.lock().expect("lock");
        let err = map.get(&missing.session_id).expect_err("unknown");
        assert!(err.to_string().contains("kernel-missing"));
    }
}
