//! Notification action routing stub — routes notification taps to desktop
//! semantic commands without duplicating capture ingest in Swift.
//!
//! Native UNUserNotificationCenter posting is deferred; ingest posts log the
//! open URL and `handle_action` routes `notification.capture.open` to the same
//! `open-resource` surface used by deep links.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const ACTION_CAPTURE_OPEN: &str = "notification.capture.open";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenResourcePayload {
    root: String,
    path: String,
}

/// Route a notification action id to the desktop command surface.
pub fn handle_action(app: &AppHandle, action: &str, context: &serde_json::Value) {
    match action {
        ACTION_CAPTURE_OPEN => {
            let root = context.get("root").and_then(|value| value.as_str()).unwrap_or("");
            let path = context.get("path").and_then(|value| value.as_str()).unwrap_or("");
            if root.is_empty() || path.is_empty() {
                eprintln!("lattice: notification action {action} missing root/path");
                return;
            }
            open_resource(app, root, path);
        }
        other => eprintln!("lattice: unknown notification action {other}"),
    }
}

fn open_resource(app: &AppHandle, root: &str, path: &str) {
    crate::tray::show_main_window(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(
            "open-resource",
            OpenResourcePayload {
                root: root.to_string(),
                path: path.to_string(),
            },
        );
    }
}

pub fn build_capture_open_url(root: &str, page_path: &str) -> String {
    let mut url = url::Url::parse("lattice://open").expect("valid lattice open scheme");
    url.query_pairs_mut()
        .append_pair("root", root)
        .append_pair("path", page_path);
    url.to_string()
}

/// Log a capture-ingested notification stub (UNUserNotificationCenter wiring is follow-up).
pub fn post_capture_ingested(app: &AppHandle, root: &str, page_path: &str) {
    let open_url = build_capture_open_url(root, page_path);
    eprintln!(
        "lattice notification stub: screen clip saved to {page_path} — open via {open_url}"
    );
    let _ = app;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_open_url_encodes_query() {
        let url = build_capture_open_url("/tmp/ws", "Inbox/2026-07-31.md");
        assert_eq!(
            url,
            "lattice://open?root=%2Ftmp%2Fws&path=Inbox%2F2026-07-31.md"
        );
    }

    #[test]
    fn capture_open_action_id_is_stable() {
        assert_eq!(ACTION_CAPTURE_OPEN, "notification.capture.open");
    }
}
