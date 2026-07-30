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
    pub ingested_at_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureShelfSnapshot {
    pub entries: Vec<CaptureShelfEntry>,
    pub count: usize,
    pub latest_title: Option<String>,
}

#[derive(Default)]
pub struct CaptureShelfState {
    inner: Mutex<VecDeque<CaptureShelfEntry>>,
}

impl CaptureShelfState {
    pub fn push(&self, page_path: String) -> CaptureShelfSnapshot {
        let entry = CaptureShelfEntry {
            page_path,
            ingested_at_ms: now_ms(),
        };
        let mut guard = self.inner.lock().expect("capture shelf lock");
        guard.push_front(entry);
        while guard.len() > MAX_ENTRIES {
            guard.pop_back();
        }
        snapshot_from(&guard)
    }

    pub fn snapshot(&self) -> CaptureShelfSnapshot {
        let guard = self.inner.lock().expect("capture shelf lock");
        snapshot_from(&guard)
    }
}

fn snapshot_from(entries: &VecDeque<CaptureShelfEntry>) -> CaptureShelfSnapshot {
    CaptureShelfSnapshot {
        count: entries.len(),
        latest_title: entries.front().map(|entry| clip_title(&entry.page_path)),
        entries: entries.iter().cloned().collect(),
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

pub fn on_ingested(app: &AppHandle, page_path: String) {
    let snapshot = app.state::<CaptureShelfState>().push(page_path);
    let _ = app.emit(CAPTURE_SHELF_UPDATED_EVENT, &snapshot);
    reveal_shelf_window(app, &snapshot);
    crate::tray::refresh_from_workflows(app);
}

pub fn show_shelf_window(app: &AppHandle) {
    let snapshot = app.state::<CaptureShelfState>().snapshot();
    reveal_shelf_window(app, &snapshot);
}

fn reveal_shelf_window(app: &AppHandle, snapshot: &CaptureShelfSnapshot) {
    let Some(window) = app.get_webview_window(CAPTURE_SHELF_WINDOW_LABEL) else {
        return;
    };
    position_shelf_window(&window);
    let _ = window.emit(CAPTURE_SHELF_UPDATED_EVENT, snapshot);
    let _ = window.show();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_latest_entries() {
        let state = CaptureShelfState::default();
        for index in 0..25 {
            state.push(format!("Inbox/clip-{index}.md"));
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.count, MAX_ENTRIES);
        assert_eq!(snapshot.latest_title.as_deref(), Some("clip-24.md"));
        assert_eq!(snapshot.entries.len(), MAX_ENTRIES);
        assert_eq!(snapshot.entries[0].page_path, "Inbox/clip-24.md");
        assert_eq!(snapshot.entries[MAX_ENTRIES - 1].page_path, "Inbox/clip-5.md");
    }
}
