//! Tauri-side wiring for the `lattice-core` workspace watcher.
//!
//! Keeps `lattice-core` free of Tauri (docs/27): this module owns the one
//! active [`WorkspaceWatcher`], forwards its events to the frontend as the
//! `workspace-changed` event, and applies `.md` changes to the workspace
//! search index — the index dependency lives here, not in core.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lattice_core::{ResourceKind, WorkspaceEvent, WorkspaceWatcher};
use lattice_index::{upsert_page, WorkspaceIndex};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const WORKSPACE_CHANGED_EVENT: &str = "workspace-changed";

/// Tauri-managed slot for the watcher of the currently open workspace. Only
/// one workspace is open at a time in v0, so this holds at most one entry;
/// opening a new workspace stops and replaces whatever was here.
#[derive(Default)]
pub struct WatcherState(Mutex<Option<WorkspaceWatcher>>);

/// Wire-format mirror of [`WorkspaceEvent`] sent to the frontend as the
/// `workspace-changed` event payload. Serialization matches
/// `lattice-commands`' kebab-case `type`-tag convention.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum WorkspaceChangePayload {
    Created {
        path: String,
        revision: String,
    },
    Modified {
        path: String,
        revision: String,
    },
    Deleted {
        path: String,
    },
    Renamed {
        from: String,
        to: String,
        revision: String,
    },
}

impl WorkspaceChangePayload {
    fn from_event(event: &WorkspaceEvent) -> Self {
        match event {
            WorkspaceEvent::Created { path, revision } => WorkspaceChangePayload::Created {
                path: path_string(path),
                revision: revision.clone(),
            },
            WorkspaceEvent::Modified { path, revision } => WorkspaceChangePayload::Modified {
                path: path_string(path),
                revision: revision.clone(),
            },
            WorkspaceEvent::Deleted { path } => WorkspaceChangePayload::Deleted {
                path: path_string(path),
            },
            WorkspaceEvent::Renamed { from, to, revision } => WorkspaceChangePayload::Renamed {
                from: path_string(from),
                to: path_string(to),
                revision: revision.clone(),
            },
        }
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Start watching `root`, stopping any watcher already active for a
/// previous workspace first.
///
/// Failure to start is non-fatal — the workspace stays usable, just
/// without live external-edit reconciliation — so this logs and returns
/// rather than surfacing an error to the caller.
#[tauri::command]
pub fn start_watching(root: String, app: AppHandle, state: tauri::State<WatcherState>) {
    start(app, &state, PathBuf::from(root));
}

/// Stop the active watcher, if any.
#[tauri::command]
pub fn stop_watching(state: tauri::State<WatcherState>) {
    stop(&state);
}

fn start(app: AppHandle, state: &WatcherState, root: PathBuf) {
    stop(state);

    let index = match WorkspaceIndex::open(&root) {
        Ok(index) => Some(index),
        Err(err) => {
            eprintln!("lattice: failed to open workspace index for watcher: {err}");
            None
        }
    };

    let (watcher, events) = match WorkspaceWatcher::start(root.clone()) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("lattice: failed to start workspace watcher: {err}");
            return;
        }
    };

    std::thread::spawn(move || {
        for event in events {
            if let Some(index) = &index {
                apply_to_index(index, &event, &root);
            }
            let payload = WorkspaceChangePayload::from_event(&event);
            if let Err(err) = app.emit(WORKSPACE_CHANGED_EVENT, payload) {
                eprintln!("lattice: failed to emit workspace-changed: {err}");
            }
        }
    });

    *state.0.lock().unwrap() = Some(watcher);
}

/// The forwarding thread started in [`start`] exits on its own once the
/// watcher's event channel disconnects, so this only needs to drop the
/// watcher handle to stop the underlying OS watch.
fn stop(state: &WatcherState) {
    if let Some(watcher) = state.0.lock().unwrap().take() {
        watcher.stop();
    }
}

fn apply_to_index(index: &WorkspaceIndex, event: &WorkspaceEvent, root: &Path) {
    match event {
        WorkspaceEvent::Created { path, .. } | WorkspaceEvent::Modified { path, .. } => {
            reindex_if_page(index, root, path);
        }
        WorkspaceEvent::Deleted { path } => remove_if_page(index, path),
        WorkspaceEvent::Renamed { from, to, .. } => {
            remove_if_page(index, from);
            reindex_if_page(index, root, to);
        }
    }
}

fn is_page(path: &Path) -> bool {
    ResourceKind::classify(path, false) == ResourceKind::Page
}

fn remove_if_page(index: &WorkspaceIndex, path: &Path) {
    if !is_page(path) {
        return;
    }
    if let Err(err) = index.remove_resource(path) {
        eprintln!(
            "lattice: failed to remove {} from index: {err}",
            path.display()
        );
    }
}

fn reindex_if_page(index: &WorkspaceIndex, root: &Path, path: &Path) {
    if !is_page(path) {
        return;
    }
    match std::fs::read_to_string(root.join(path)) {
        Ok(content) => {
            if let Err(err) = upsert_page(index, path, &content) {
                eprintln!("lattice: failed to index {}: {err}", path.display());
            }
        }
        Err(err) => {
            // Benign race: the file was replaced or removed again before we
            // read it. The next settled event will re-index it.
            eprintln!(
                "lattice: skipped indexing {} after watch event: {err}",
                path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_from_event_preserves_forward_slash_paths() {
        let event = WorkspaceEvent::Renamed {
            from: PathBuf::from("Notes/Old.md"),
            to: PathBuf::from("Notes/New.md"),
            revision: "sha256:abc".to_string(),
        };
        let payload = WorkspaceChangePayload::from_event(&event);
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["type"], "renamed");
        assert_eq!(json["from"], "Notes/Old.md");
        assert_eq!(json["to"], "Notes/New.md");
        assert_eq!(json["revision"], "sha256:abc");
    }

    #[test]
    fn is_page_matches_markdown_only() {
        assert!(is_page(Path::new("Notes/Idea.md")));
        assert!(!is_page(Path::new("Board.canvas")));
        assert!(!is_page(Path::new(".lattice/index.sqlite")));
    }

    #[test]
    fn reindex_if_page_is_a_noop_for_non_pages() {
        let dir = tempfile::tempdir().unwrap();
        lattice_core::Workspace::init(dir.path(), "Test").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        // No file exists at this path; a page would error attempting to
        // read it, but a non-page path must return before ever trying.
        reindex_if_page(&index, dir.path(), Path::new("missing.canvas"));
    }
}
