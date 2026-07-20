//! Semantic search enable / status for the desktop shell (E4).
//!
//! Desktop workspace search uses the embedded [`lattice_runtime`] path today, so
//! enable/disable/status go through [`lattice_handlers`] on that same runtime.
//! Daemon clients use the EnableSemanticSearch / GetSemanticStatus wire RPCs
//! (distinct from voice PrepareModel).

use lattice_runtime::{SemanticStatus, SemanticStatusState};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

const SEMANTIC_EVENT: &str = "semantic-event";

#[derive(Default)]
pub struct SemanticState {
    inner: Mutex<SemanticInner>,
}

#[derive(Default)]
struct SemanticInner {
    /// Last workspace root used for enable/status (Settings + shell).
    root: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStatusDto {
    pub state: String,
    pub pending_chunks: Option<u64>,
    pub message: Option<String>,
}

impl From<SemanticStatus> for SemanticStatusDto {
    fn from(value: SemanticStatus) -> Self {
        Self {
            state: value.state.as_str().to_string(),
            pending_chunks: value.pending_chunks,
            message: value.message,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SemanticUiEvent {
    #[serde(rename_all = "camelCase")]
    Status {
        state: String,
        pending_chunks: Option<u64>,
        message: Option<String>,
    },
}

fn emit_status(app: &AppHandle, status: &SemanticStatusDto) {
    let _ = app.emit(
        SEMANTIC_EVENT,
        SemanticUiEvent::Status {
            state: status.state.clone(),
            pending_chunks: status.pending_chunks,
            message: status.message.clone(),
        },
    );
}

fn map_status(status: SemanticStatus) -> SemanticStatusDto {
    SemanticStatusDto::from(status)
}

#[tauri::command]
pub async fn semantic_status(
    root: String,
    state: State<'_, SemanticState>,
) -> Result<SemanticStatusDto, String> {
    {
        let mut inner = state.inner.lock().await;
        inner.root = Some(root.clone());
    }
    let status = lattice_handlers::semantic_search_status(root)?;
    Ok(map_status(status))
}

#[tauri::command]
pub async fn semantic_enable(
    app: AppHandle,
    root: String,
    state: State<'_, SemanticState>,
) -> Result<SemanticStatusDto, String> {
    {
        let mut inner = state.inner.lock().await;
        inner.root = Some(root.clone());
    }
    let status = lattice_handlers::enable_semantic_search(root)?;
    let dto = map_status(status);
    emit_status(&app, &dto);
    Ok(dto)
}

#[tauri::command]
pub async fn semantic_disable(
    app: AppHandle,
    root: String,
    state: State<'_, SemanticState>,
) -> Result<SemanticStatusDto, String> {
    {
        let mut inner = state.inner.lock().await;
        inner.root = Some(root.clone());
    }
    let status = lattice_handlers::disable_semantic_search(root)?;
    let dto = map_status(status);
    emit_status(&app, &dto);
    Ok(dto)
}

/// Map a wire/state string into a Settings label (pure helper for tests).
pub fn status_label(state: &str, pending_chunks: Option<u64>) -> String {
    let parsed = SemanticStatusState::parse(state);
    match parsed {
        Some(SemanticStatusState::Stopped) => "Not prepared".into(),
        Some(SemanticStatusState::Preparing) => "Preparing…".into(),
        Some(SemanticStatusState::Indexing) => match pending_chunks {
            Some(n) if n > 0 => format!("Indexing ({n} pending)"),
            _ => "Indexing…".into(),
        },
        Some(SemanticStatusState::Ready) => "Ready".into(),
        Some(SemanticStatusState::Degraded) => "Degraded (keyword search still works)".into(),
        Some(SemanticStatusState::Failed) => "Failed".into(),
        None => format!("Unknown ({state})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_label_covers_all_states() {
        assert_eq!(status_label("stopped", None), "Not prepared");
        assert_eq!(status_label("preparing", None), "Preparing…");
        assert_eq!(status_label("indexing", Some(3)), "Indexing (3 pending)");
        assert_eq!(status_label("ready", Some(0)), "Ready");
        assert!(status_label("degraded", None).contains("Degraded"));
        assert_eq!(status_label("failed", None), "Failed");
    }
}
