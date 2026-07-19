//! Tauri-side wiring for the embedded terminal (ADR 0039).
//!
//! Keeps `lattice-terminal` free of Tauri: this module owns session state,
//! capability gating, and event emission to the webview.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use lattice_core::Workspace;
use lattice_terminal::{SpawnOptions, TerminalSession};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const TERMINAL_OUTPUT_EVENT: &str = "terminal-output";
const TERMINAL_EXIT_EVENT: &str = "terminal-exit";

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Tauri-managed map of live PTY sessions keyed by opaque session id.
///
/// v1 keeps at most one session: a new spawn clears any previous entry.
#[derive(Default)]
pub struct TerminalState(Mutex<HashMap<String, TerminalSession>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputPayload {
    session_id: String,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalExitPayload {
    session_id: String,
    code: Option<i32>,
}

/// Open `root` and require `"terminal"` in `lattice.yaml` capabilities.enabled.
fn require_terminal_capability(root: &Path) -> Result<Workspace, String> {
    let workspace = Workspace::open(root).map_err(|err| err.to_string())?;
    if !workspace
        .manifest()
        .capabilities
        .enabled
        .iter()
        .any(|capability| capability == "terminal")
    {
        return Err("terminal capability is not enabled".into());
    }
    Ok(workspace)
}

fn next_session_id() -> String {
    format!("term-{}", NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed))
}

fn unknown_session(session_id: &str) -> String {
    format!("unknown terminal session: {session_id}")
}

fn clear_sessions(state: &TerminalState) {
    let mut sessions = state.0.lock().expect("terminal state lock");
    for (_, mut session) in sessions.drain() {
        let _ = session.kill();
    }
}

fn start_output_forwarder(
    app: AppHandle,
    session_id: String,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
) {
    std::thread::spawn(move || {
        for chunk in rx {
            let payload = TerminalOutputPayload {
                session_id: session_id.clone(),
                data: chunk,
            };
            if let Err(err) = app.emit(TERMINAL_OUTPUT_EVENT, &payload) {
                eprintln!("lattice: failed to emit terminal-output: {err}");
                break;
            }
        }
        let payload = TerminalExitPayload {
            session_id,
            // portable-pty exit status is not plumbed through TerminalSession yet.
            code: None,
        };
        if let Err(err) = app.emit(TERMINAL_EXIT_EVENT, &payload) {
            eprintln!("lattice: failed to emit terminal-exit: {err}");
        }
    });
}

/// Spawn a PTY shell at the workspace root when the terminal capability is enabled.
#[tauri::command]
pub fn terminal_spawn(
    root: String,
    cols: u16,
    rows: u16,
    app: AppHandle,
    state: tauri::State<'_, TerminalState>,
) -> Result<SpawnResult, String> {
    spawn_session(&app, &state, root, cols, rows)
}

fn spawn_session(
    app: &AppHandle,
    state: &TerminalState,
    root: String,
    cols: u16,
    rows: u16,
) -> Result<SpawnResult, String> {
    let workspace = require_terminal_capability(Path::new(&root))?;
    // Single-session v1: replace any prior PTY.
    clear_sessions(state);

    let (session, rx) = TerminalSession::spawn(SpawnOptions::new(workspace.root(), cols, rows))
        .map_err(|err| err.to_string())?;
    let session_id = next_session_id();
    start_output_forwarder(app.clone(), session_id.clone(), rx);

    state
        .0
        .lock()
        .expect("terminal state lock")
        .insert(session_id.clone(), session);

    Ok(SpawnResult { session_id })
}

/// Write UTF-8 (or raw) input bytes to a live session.
#[tauri::command]
pub fn terminal_write(
    session_id: String,
    data: String,
    state: tauri::State<'_, TerminalState>,
) -> Result<(), String> {
    write_session(&state, &session_id, data.as_bytes())
}

fn write_session(state: &TerminalState, session_id: &str, data: &[u8]) -> Result<(), String> {
    let mut sessions = state.0.lock().expect("terminal state lock");
    let session = sessions
        .get_mut(session_id)
        .ok_or_else(|| unknown_session(session_id))?;
    session.write(data).map_err(|err| err.to_string())
}

/// Resize a live PTY window.
#[tauri::command]
pub fn terminal_resize(
    session_id: String,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, TerminalState>,
) -> Result<(), String> {
    resize_session(&state, &session_id, cols, rows)
}

fn resize_session(
    state: &TerminalState,
    session_id: &str,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut sessions = state.0.lock().expect("terminal state lock");
    let session = sessions
        .get_mut(session_id)
        .ok_or_else(|| unknown_session(session_id))?;
    session.resize(cols, rows).map_err(|err| err.to_string())
}

/// Kill a session and remove it from managed state.
#[tauri::command]
pub fn terminal_kill(
    session_id: String,
    state: tauri::State<'_, TerminalState>,
) -> Result<(), String> {
    kill_session(&state, &session_id)
}

fn kill_session(state: &TerminalState, session_id: &str) -> Result<(), String> {
    let mut sessions = state.0.lock().expect("terminal state lock");
    let mut session = sessions
        .remove(session_id)
        .ok_or_else(|| unknown_session(session_id))?;
    session.kill().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::{Workspace, WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME};

    fn init_workspace_without_terminal() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        Workspace::init(dir.path(), "Terminal Test").expect("init workspace");
        dir
    }

    fn enable_terminal(root: &Path) {
        let manifest_path = root.join(WORKSPACE_MANIFEST_FILENAME);
        let mut manifest = WorkspaceManifest::load(&manifest_path).expect("load manifest");
        if !manifest
            .capabilities
            .enabled
            .iter()
            .any(|capability| capability == "terminal")
        {
            manifest.capabilities.enabled.push("terminal".into());
        }
        manifest.save(&manifest_path).expect("save manifest");
    }

    #[test]
    fn spawn_denies_when_terminal_capability_missing() {
        let dir = init_workspace_without_terminal();
        let err = require_terminal_capability(dir.path()).expect_err("deny");
        assert_eq!(err, "terminal capability is not enabled");
    }

    #[test]
    fn spawn_gate_accepts_enabled_terminal_capability() {
        let dir = init_workspace_without_terminal();
        enable_terminal(dir.path());
        let workspace = require_terminal_capability(dir.path()).expect("allow");
        assert_eq!(workspace.root(), dir.path());
    }

    #[test]
    fn write_resize_kill_reject_unknown_session_id() {
        let state = TerminalState::default();
        let missing = "term-missing";

        let write_err = write_session(&state, missing, b"echo hi\n").expect_err("write");
        assert!(write_err.contains(missing));

        let resize_err = resize_session(&state, missing, 80, 24).expect_err("resize");
        assert!(resize_err.contains(missing));

        let kill_err = kill_session(&state, missing).expect_err("kill");
        assert!(kill_err.contains(missing));
    }

    #[test]
    fn capability_present_can_spawn_and_kill_session() {
        let dir = init_workspace_without_terminal();
        enable_terminal(dir.path());
        let workspace = require_terminal_capability(dir.path()).expect("capability");

        let (mut session, _rx) =
            TerminalSession::spawn(SpawnOptions::new(workspace.root(), 80, 24))
                .expect("spawn with terminal capability");
        session.kill().expect("kill");
        assert!(!session.is_alive());
    }

    #[test]
    fn spawn_result_serializes_camel_case_session_id() {
        let json = serde_json::to_value(SpawnResult {
            session_id: "term-1".into(),
        })
        .expect("serialize");
        assert_eq!(json["sessionId"], "term-1");
        assert!(json.get("session_id").is_none());
    }
}
