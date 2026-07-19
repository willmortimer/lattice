//! Apply debounced [`WorkspaceEvent`]s to a warm [`WorkspaceIndex`].
//!
//! Shared by daemon/runtime watchers so Tauri can later become a subscriber
//! instead of owning index maintenance. Link-repair side effects stay in the
//! desktop shell for now.

use std::path::Path;

use lattice_core::{Resource, ResourceKind, WorkspaceEvent};
use lattice_index::WorkspaceIndex;

/// Outcome of applying one workspace filesystem event to the warm index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexApplyOutcome {
    /// Event ignored (e.g. root deleted, non-resource path).
    Ignored,
    /// Resource rows upserted for `path`.
    Upserted { path: std::path::PathBuf },
    /// Resource rows removed for `path`.
    Removed { path: std::path::PathBuf },
    /// Rename handled as remove `from` + upsert `to`.
    Renamed {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
    },
    /// Index mutation failed (benign races are reported as skipped).
    Skipped {
        path: std::path::PathBuf,
        reason: String,
    },
}

/// Upsert/remove index rows for a settled [`WorkspaceEvent`].
///
/// Returns one outcome describing the index mutation (if any). Does not
/// perform a full rebuild.
pub fn apply_workspace_event_to_index(
    index: &WorkspaceIndex,
    event: &WorkspaceEvent,
) -> IndexApplyOutcome {
    match event {
        WorkspaceEvent::RootDeleted => IndexApplyOutcome::Ignored,
        WorkspaceEvent::Created { path, .. } | WorkspaceEvent::Modified { path, .. } => {
            match reindex_if_resource(index, path) {
                Ok(true) => IndexApplyOutcome::Upserted {
                    path: path.to_path_buf(),
                },
                Ok(false) => IndexApplyOutcome::Ignored,
                Err(reason) => IndexApplyOutcome::Skipped {
                    path: path.to_path_buf(),
                    reason,
                },
            }
        }
        WorkspaceEvent::Deleted { path } => match remove_if_resource(index, path) {
            Ok(true) => IndexApplyOutcome::Removed {
                path: path.to_path_buf(),
            },
            Ok(false) => IndexApplyOutcome::Ignored,
            Err(reason) => IndexApplyOutcome::Skipped {
                path: path.to_path_buf(),
                reason,
            },
        },
        WorkspaceEvent::Renamed { from, to, .. } => {
            let _ = remove_if_resource(index, from);
            match reindex_if_resource(index, to) {
                Ok(true) => IndexApplyOutcome::Renamed {
                    from: from.to_path_buf(),
                    to: to.to_path_buf(),
                },
                Ok(false) => {
                    // Destination is not a tracked resource; still report removal.
                    IndexApplyOutcome::Removed {
                        path: from.to_path_buf(),
                    }
                }
                Err(reason) => IndexApplyOutcome::Skipped {
                    path: to.to_path_buf(),
                    reason,
                },
            }
        }
    }
}

/// Classify a relative path the same way the Tauri watcher did: skip
/// `.`-prefixed segments and package-directory children.
pub fn resource_for_event(path: &Path) -> Option<Resource> {
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

fn remove_if_resource(index: &WorkspaceIndex, path: &Path) -> std::result::Result<bool, String> {
    if resource_for_event(path).is_none() {
        return Ok(false);
    }
    index
        .remove_resource(path)
        .map(|_| true)
        .map_err(|err| err.to_string())
}

fn reindex_if_resource(index: &WorkspaceIndex, path: &Path) -> std::result::Result<bool, String> {
    let Some(resource) = resource_for_event(path) else {
        return Ok(false);
    };
    index
        .upsert_resource(&resource)
        .map(|_| true)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use std::path::PathBuf;

    #[test]
    fn apply_updates_generic_resources_and_removes_stale_rows() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "first searchable value").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();

        let created = apply_workspace_event_to_index(
            &index,
            &WorkspaceEvent::Created {
                path: PathBuf::from("notes.txt"),
                revision: "external".into(),
            },
        );
        assert_eq!(
            created,
            IndexApplyOutcome::Upserted {
                path: PathBuf::from("notes.txt")
            }
        );
        assert!(index
            .search("first searchable", 10)
            .unwrap()
            .iter()
            .any(|hit| hit.path == Path::new("notes.txt")));

        std::fs::write(dir.path().join("notes.txt"), "second searchable value").unwrap();
        assert_eq!(
            apply_workspace_event_to_index(
                &index,
                &WorkspaceEvent::Modified {
                    path: PathBuf::from("notes.txt"),
                    revision: "external-2".into(),
                },
            ),
            IndexApplyOutcome::Upserted {
                path: PathBuf::from("notes.txt")
            }
        );
        assert!(index.search("first searchable", 10).unwrap().is_empty());
        assert!(index
            .search("second searchable", 10)
            .unwrap()
            .iter()
            .any(|hit| hit.path == Path::new("notes.txt")));

        std::fs::rename(dir.path().join("notes.txt"), dir.path().join("renamed.txt")).unwrap();
        assert_eq!(
            apply_workspace_event_to_index(
                &index,
                &WorkspaceEvent::Renamed {
                    from: PathBuf::from("notes.txt"),
                    to: PathBuf::from("renamed.txt"),
                    revision: "external-3".into(),
                },
            ),
            IndexApplyOutcome::Renamed {
                from: PathBuf::from("notes.txt"),
                to: PathBuf::from("renamed.txt"),
            }
        );
        assert!(index.metadata(Path::new("notes.txt")).unwrap().is_none());
        assert!(index
            .search("second searchable", 10)
            .unwrap()
            .iter()
            .any(|hit| hit.path == Path::new("renamed.txt")));

        std::fs::remove_file(dir.path().join("renamed.txt")).unwrap();
        assert_eq!(
            apply_workspace_event_to_index(
                &index,
                &WorkspaceEvent::Deleted {
                    path: PathBuf::from("renamed.txt"),
                },
            ),
            IndexApplyOutcome::Removed {
                path: PathBuf::from("renamed.txt")
            }
        );
        assert!(index.metadata(Path::new("renamed.txt")).unwrap().is_none());
    }

    #[test]
    fn package_children_are_not_indexed_as_separate_resources() {
        assert!(resource_for_event(Path::new("Notes/Idea.md")).is_some());
        assert!(resource_for_event(Path::new("CRM.data/app.yaml")).is_none());
    }
}
