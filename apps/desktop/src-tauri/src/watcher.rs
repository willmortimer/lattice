//! Tauri-side wiring for the `lattice-core` workspace watcher.
//!
//! Keeps `lattice-core` free of Tauri (docs/27): this module owns the one
//! active [`WorkspaceWatcher`], forwards its events to the frontend as the
//! `workspace-changed` event, and applies supported-file changes to the workspace
//! search index — the index dependency lives here, not in core.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lattice_core::{Resource, ResourceKind, WorkspaceEvent, WorkspaceWatcher};
use lattice_index::WorkspaceIndex;
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
        WorkspaceEvent::RootDeleted => {}
        WorkspaceEvent::Created { path, .. } | WorkspaceEvent::Modified { path, .. } => {
            reindex_if_resource(index, root, path);
        }
        WorkspaceEvent::Deleted { path } => remove_if_resource(index, path),
        WorkspaceEvent::Renamed { from, to, .. } => {
            remove_if_resource(index, from);
            reindex_if_resource(index, root, to);
        }
    }
}

#[cfg(test)]
fn is_page(path: &Path) -> bool {
    resource_for_event(path).is_some_and(|resource| resource.kind == ResourceKind::Page)
}

fn resource_for_event(path: &Path) -> Option<Resource> {
    // Workspace::scan yields package directories as one resource and does not
    // expose their children. Keep this check local; do not scan the workspace
    // synchronously for every event.
    if path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with('.'))
    }) {
        return None;
    }
    if path
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| component.as_os_str().to_str())
        .any(is_package_directory_name)
    {
        return None;
    }
    Some(Resource {
        path: path.to_path_buf(),
        kind: ResourceKind::classify(path, false),
    })
}

fn is_package_directory_name(name: &str) -> bool {
    matches!(
        name.rsplit_once('.').map(|(_, extension)| extension),
        Some("data" | "dataset" | "ink" | "artifact" | "app" | "task")
    )
}

fn remove_if_resource(index: &WorkspaceIndex, path: &Path) {
    if resource_for_event(path).is_none() {
        return;
    }
    if let Err(err) = index.remove_resource(path) {
        eprintln!(
            "lattice: failed to remove {} from index: {err}",
            path.display()
        );
    }
}

fn reindex_if_resource(index: &WorkspaceIndex, _root: &Path, path: &Path) {
    let Some(resource) = resource_for_event(path) else {
        return;
    };
    if let Err(err) = index.upsert_resource(&resource) {
        // Benign race: the file was replaced or removed again before the
        // bounded runtime probe completed. The next settled event retries it.
        eprintln!(
            "lattice: skipped indexing {} after watch event: {err}",
            path.display()
        );
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
    fn reindex_if_resource_is_a_noop_for_missing_non_page() {
        let dir = tempfile::tempdir().unwrap();
        lattice_core::Workspace::init(dir.path(), "Test").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        // No file exists at this path; a page would error attempting to
        // read it, but a non-page path must return before ever trying.
        reindex_if_resource(&index, dir.path(), Path::new("missing.canvas"));
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
        assert!(resource_for_event(Path::new("Notes/Idea.md")).is_some());
        assert!(resource_for_event(Path::new("CRM.data/app.yaml")).is_none());
    }
}
