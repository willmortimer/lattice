//! Tauri-side wiring for the `lattice-core` workspace watcher.
//!
//! Keeps `lattice-core` free of Tauri (docs/27): this module owns the one
//! active [`WorkspaceWatcher`], forwards its events to the frontend as the
//! `workspace-changed` event, and applies supported-file changes to the workspace
//! search index — the index dependency lives here, not in core.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lattice_core::{WorkspaceEvent, WorkspaceWatcher};
#[cfg(test)]
use lattice_core::ResourceKind;
use lattice_index::WorkspaceIndex;
use lattice_runtime::apply_workspace_event_to_index;
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
    WorkspaceUnavailable {
        reason: String,
    },
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
            WorkspaceEvent::RootDeleted => WorkspaceChangePayload::WorkspaceUnavailable {
                reason: "root-deleted".into(),
            },
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
            match &event {
                WorkspaceEvent::Created { path, .. }
                | WorkspaceEvent::Modified { path, .. }
                | WorkspaceEvent::Deleted { path } => {
                    crate::workflow::on_resource_changed(&app, &root, &path_string(path));
                }
                WorkspaceEvent::Renamed { from, to, .. } => {
                    crate::workflow::on_resource_changed(&app, &root, &path_string(from));
                    crate::workflow::on_resource_changed(&app, &root, &path_string(to));
                }
                WorkspaceEvent::RootDeleted => {}
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
    if let WorkspaceEvent::Renamed { from, to, .. } = event {
        // Capture repair candidates before removing the old index rows;
        // inbound link spans still name `from` until pages are rewritten.
        if let Err(err) = crate::link_repair::save_external_link_repair_proposal(root, from, to) {
            eprintln!("lattice: failed to save external link-repair proposal: {err}");
        }
    }
    let _ = apply_workspace_event_to_index(index, event);
}

#[cfg(test)]
fn is_page(path: &Path) -> bool {
    lattice_runtime::resource_for_event(path)
        .is_some_and(|resource| resource.kind == ResourceKind::Page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
    fn root_deletion_becomes_workspace_unavailable() {
        let payload = WorkspaceChangePayload::from_event(&WorkspaceEvent::RootDeleted);
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["type"], "workspace-unavailable");
        assert_eq!(json["reason"], "root-deleted");
    }

    #[test]
    fn is_page_matches_markdown_only() {
        assert!(is_page(Path::new("Notes/Idea.md")));
        assert!(!is_page(Path::new("Board.canvas")));
        assert!(!is_page(Path::new(".lattice/index.sqlite")));
    }

    #[test]
    fn watcher_dispatch_updates_generic_resources_and_removes_stale_rows() {
        let dir = tempfile::tempdir().unwrap();
        lattice_core::Workspace::init(dir.path(), "Test").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "first searchable value").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();

        apply_to_index(
            &index,
            &WorkspaceEvent::Created {
                path: PathBuf::from("notes.txt"),
                revision: "external".to_string(),
            },
            dir.path(),
        );
        assert!(index
            .search("first searchable", 10)
            .unwrap()
            .iter()
            .any(|hit| { hit.path == Path::new("notes.txt") }));

        std::fs::write(dir.path().join("notes.txt"), "second searchable value").unwrap();
        apply_to_index(
            &index,
            &WorkspaceEvent::Modified {
                path: PathBuf::from("notes.txt"),
                revision: "external-2".to_string(),
            },
            dir.path(),
        );
        assert!(index
            .search("second searchable", 10)
            .unwrap()
            .iter()
            .any(|hit| { hit.path == Path::new("notes.txt") }));
        assert!(index.search("first searchable", 10).unwrap().is_empty());

        std::fs::rename(dir.path().join("notes.txt"), dir.path().join("renamed.txt")).unwrap();
        apply_to_index(
            &index,
            &WorkspaceEvent::Renamed {
                from: PathBuf::from("notes.txt"),
                to: PathBuf::from("renamed.txt"),
                revision: "external-3".to_string(),
            },
            dir.path(),
        );
        assert!(index.metadata(Path::new("notes.txt")).unwrap().is_none());
        assert!(index
            .search("second searchable", 10)
            .unwrap()
            .iter()
            .any(|hit| { hit.path == Path::new("renamed.txt") }));

        std::fs::remove_file(dir.path().join("renamed.txt")).unwrap();
        apply_to_index(
            &index,
            &WorkspaceEvent::Deleted {
                path: PathBuf::from("renamed.txt"),
            },
            dir.path(),
        );
        assert!(index.metadata(Path::new("renamed.txt")).unwrap().is_none());
    }

    #[test]
    fn package_children_are_not_indexed_as_separate_resources() {
        assert!(lattice_runtime::resource_for_event(Path::new("Notes/Idea.md")).is_some());
        assert!(lattice_runtime::resource_for_event(Path::new("CRM.data/app.yaml")).is_none());
    }
}
