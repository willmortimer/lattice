//! Non-activating capture shelf fed by `capture-ingested` events.
//!
//! Windows `WDA_EXCLUDEFROMCAPTURE` exclusion is deferred (see ADR-0052).

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, State, WebviewWindow};

pub const CAPTURE_SHELF_WINDOW_LABEL: &str = "capture-shelf";
pub const CAPTURE_SHELF_UPDATED_EVENT: &str = "capture-shelf-updated";

const MAX_ENTRIES: usize = 20;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureShelfEntry {
    pub page_path: String,
    pub title: String,
    pub ingested_at_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureShelfSnapshot {
    pub entries: Vec<CaptureShelfEntry>,
    pub count: usize,
    pub latest_title: Option<String>,
    pub destination_directory: Option<String>,
    pub workspace_root: Option<String>,
}

#[derive(Default)]
pub struct CaptureShelfState {
    inner: Mutex<VecDeque<CaptureShelfEntry>>,
    destination_directory: Mutex<Option<String>>,
    workspace_root: Mutex<Option<String>>,
}

enum ShelfRevealMode {
    /// After ingest: float above other apps without stealing focus.
    Passive,
    /// Tray/menu action: user explicitly opened the shelf.
    Activating,
}

impl CaptureShelfState {
    pub fn record_ingest(
        &self,
        page_path: String,
        workspace_root: String,
        destination_directory: String,
    ) -> CaptureShelfSnapshot {
        let entry = CaptureShelfEntry {
            title: clip_title(&page_path),
            page_path,
            ingested_at_ms: now_ms(),
        };
        {
            let mut guard = self.inner.lock().expect("capture shelf lock");
            guard.push_front(entry);
            while guard.len() > MAX_ENTRIES {
                guard.pop_back();
            }
        }
        {
            let mut destination = self
                .destination_directory
                .lock()
                .expect("capture shelf destination lock");
            *destination = Some(destination_directory);
        }
        {
            let mut root = self
                .workspace_root
                .lock()
                .expect("capture shelf workspace lock");
            *root = Some(workspace_root);
        }
        self.snapshot()
    }

    pub fn snapshot(&self) -> CaptureShelfSnapshot {
        let guard = self.inner.lock().expect("capture shelf lock");
        let destination_directory = self
            .destination_directory
            .lock()
            .expect("capture shelf destination lock")
            .clone();
        let workspace_root = self
            .workspace_root
            .lock()
            .expect("capture shelf workspace lock")
            .clone();
        snapshot_from(&guard, destination_directory, workspace_root)
    }
}

fn snapshot_from(
    entries: &VecDeque<CaptureShelfEntry>,
    destination_directory: Option<String>,
    workspace_root: Option<String>,
) -> CaptureShelfSnapshot {
    CaptureShelfSnapshot {
        count: entries.len(),
        latest_title: entries.front().map(|entry| entry.title.clone()),
        entries: entries.iter().cloned().collect(),
        destination_directory,
        workspace_root,
    }
}

fn clip_title(page_path: &str) -> String {
    Path::new(page_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(page_path)
        .to_string()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Apply floating utility-window behavior during app setup.
pub fn install_shelf_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CAPTURE_SHELF_WINDOW_LABEL) else {
        return;
    };
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);
    super::shelf_platform::configure_floating_panel(&window);
    position_shelf_window(&window);
}

pub fn on_ingested(
    app: &AppHandle,
    page_path: String,
    workspace_root: String,
    destination_directory: String,
) {
    let snapshot = app
        .state::<CaptureShelfState>()
        .record_ingest(page_path, workspace_root, destination_directory);
    let _ = app.emit(CAPTURE_SHELF_UPDATED_EVENT, &snapshot);
    reveal_shelf_window(app, &snapshot, ShelfRevealMode::Passive);
    crate::tray::refresh_from_workflows(app);
}

pub fn show_shelf_window(app: &AppHandle) {
    let snapshot = app.state::<CaptureShelfState>().snapshot();
    reveal_shelf_window(app, &snapshot, ShelfRevealMode::Activating);
}

fn reveal_shelf_window(app: &AppHandle, snapshot: &CaptureShelfSnapshot, mode: ShelfRevealMode) {
    let Some(window) = app.get_webview_window(CAPTURE_SHELF_WINDOW_LABEL) else {
        return;
    };
    position_shelf_window(&window);
    let _ = window.emit(CAPTURE_SHELF_UPDATED_EVENT, snapshot);
    match mode {
        ShelfRevealMode::Passive => {
            super::shelf_platform::show_without_activation(&window);
        }
        ShelfRevealMode::Activating => {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn position_shelf_window(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let screen = monitor.size();
    let window_size = window.outer_size().unwrap_or_default();
    let x = (screen.width as f64 / scale) - (window_size.width as f64 / scale) - 16.0;
    let y = 48.0;
    let _ = window.set_position(LogicalPosition::new(x, y));
}

#[tauri::command]
pub fn capture_shelf_snapshot(state: State<'_, CaptureShelfState>) -> CaptureShelfSnapshot {
    state.snapshot()
}

#[tauri::command]
pub fn capture_shelf_hide(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(CAPTURE_SHELF_WINDOW_LABEL)
        .ok_or_else(|| "capture shelf window unavailable".to_string())?;
    window
        .hide()
        .map_err(|err| format!("failed to hide capture shelf: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_latest_entries() {
        let state = CaptureShelfState::default();
        for index in 0..25 {
            state.record_ingest(
                format!("Inbox/clip-{index}.md"),
                "/tmp/workspace".to_string(),
                "Inbox".to_string(),
            );
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.count, MAX_ENTRIES);
        assert_eq!(snapshot.latest_title.as_deref(), Some("clip-24.md"));
        assert_eq!(snapshot.entries.len(), MAX_ENTRIES);
        assert_eq!(snapshot.entries[0].page_path, "Inbox/clip-24.md");
        assert_eq!(snapshot.entries[0].title, "clip-24.md");
        assert_eq!(snapshot.entries[MAX_ENTRIES - 1].page_path, "Inbox/clip-5.md");
        assert_eq!(snapshot.destination_directory.as_deref(), Some("Inbox"));
        assert_eq!(snapshot.workspace_root.as_deref(), Some("/tmp/workspace"));
    }
}
