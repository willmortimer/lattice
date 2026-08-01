//! Daemon-owned resource catalog deltas and paginated `list_children`.
//!
//! MVP for ADR 0079 / sprint C0: compact metadata rows keyed by stable
//! LatticeFS [`ResourceId`], directory-scoped paging, and incremental deltas
//! bridged from [`WorkspaceEvent`]. Shell projection (C1) is out of scope.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lattice_core::{Resource, ResourceKind, Workspace, WorkspaceEvent};
use lattice_runtime::{default_runtime, LatticeRuntime, WorkspaceSession};
use latticefs_core::{NamespaceRegistry, ResourceId};
use serde::{Deserialize, Serialize};

/// Default page size for [`list_children`].
pub const DEFAULT_LIST_CHILDREN_LIMIT: usize = 100;
/// Hard cap so callers cannot request unbounded scans in one round-trip.
pub const MAX_LIST_CHILDREN_LIMIT: usize = 1_000;

/// Compact catalog metadata row keyed by stable resource identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub resource_id: String,
    pub path: String,
    pub kind: ResourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub child_count: u32,
}

/// Incremental catalog mutation for shell / Query projection (C1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CatalogDelta {
    Upsert { entries: Vec<CatalogEntry> },
    Remove { resource_ids: Vec<String> },
    /// Sibling order under `parent_id` (`None` = workspace root).
    Reorder {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
        ordered_ids: Vec<String>,
    },
    Replace { entries: Vec<CatalogEntry> },
}

/// One page of direct children under a parent folder (or workspace root).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListChildrenPage {
    pub children: Vec<CatalogEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Wire payload emitted alongside `workspace-changed` as `catalog-delta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDeltaEvent {
    pub workspace_root: String,
    pub delta: CatalogDelta,
}

fn map_runtime_err(err: lattice_runtime::Error) -> String {
    err.to_string()
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Immediate parent path key, or `None` for workspace-root entries.
pub fn parent_path_of(path: &str) -> Option<String> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let parent = Path::new(trimmed).parent()?;
    let key = path_key(parent);
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

/// True when `path` is a direct child of `parent` (`None` = workspace root).
pub fn is_direct_child(path: &str, parent: Option<&str>) -> bool {
    parent_path_of(path).as_deref() == parent
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_LIST_CHILDREN_LIMIT)
        .clamp(1, MAX_LIST_CHILDREN_LIMIT)
}

/// Paginate already-filtered children using a path cursor (exclusive lower bound).
pub fn paginate_children(
    mut children: Vec<CatalogEntry>,
    cursor: Option<&str>,
    limit: usize,
) -> ListChildrenPage {
    children.sort_by(|left, right| left.path.cmp(&right.path));
    let start = match cursor.map(str::trim).filter(|value| !value.is_empty()) {
        Some(cursor) => children.partition_point(|entry| entry.path.as_str() <= cursor),
        None => 0,
    };
    let end = (start + limit).min(children.len());
    let page = children[start..end].to_vec();
    let next_cursor = if end < children.len() {
        page.last().map(|entry| entry.path.clone())
    } else {
        None
    };
    ListChildrenPage {
        children: page,
        next_cursor,
    }
}

/// Apply a delta to an id-keyed catalog map.
pub fn apply_catalog_delta(
    current: &HashMap<String, CatalogEntry>,
    delta: &CatalogDelta,
) -> HashMap<String, CatalogEntry> {
    match delta {
        CatalogDelta::Replace { entries } => {
            let mut next = HashMap::with_capacity(entries.len());
            for entry in entries {
                next.insert(entry.resource_id.clone(), entry.clone());
            }
            next
        }
        CatalogDelta::Upsert { entries } => {
            let mut next = current.clone();
            for entry in entries {
                next.insert(entry.resource_id.clone(), entry.clone());
            }
            next
        }
        CatalogDelta::Remove { resource_ids } => {
            let mut next = current.clone();
            for resource_id in resource_ids {
                next.remove(resource_id);
            }
            next
        }
        CatalogDelta::Reorder { .. } => {
            // Order is carried on the delta for consumers; the id→entry map is unchanged.
            current.clone()
        }
    }
}

fn child_counts(resources: &[Resource]) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    for resource in resources {
        if let Some(parent) = parent_path_of(&path_key(&resource.path)) {
            *counts.entry(parent).or_default() += 1;
        }
    }
    counts
}

/// Build catalog entries for a scanned resource set, minting stable ids as needed.
pub fn catalog_entries_from_resources(
    resources: &[Resource],
    registry: &mut NamespaceRegistry,
) -> Result<Vec<CatalogEntry>, String> {
    let counts = child_counts(resources);
    let mut path_to_id: HashMap<String, String> = HashMap::with_capacity(resources.len());
    for resource in resources {
        let path = path_key(&resource.path);
        let id = registry
            .ensure_local_file(&path)
            .map_err(|err| err.to_string())?
            .to_string();
        path_to_id.insert(path, id);
    }

    let mut entries = Vec::with_capacity(resources.len());
    for resource in resources {
        let path = path_key(&resource.path);
        let resource_id = path_to_id
            .get(&path)
            .cloned()
            .ok_or_else(|| format!("missing resource id for {path}"))?;
        let parent_id = parent_path_of(&path).and_then(|parent| path_to_id.get(&parent).cloned());
        entries.push(CatalogEntry {
            resource_id,
            path: path.clone(),
            kind: resource.kind,
            parent_id,
            child_count: counts.get(&path).copied().unwrap_or(0),
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn classify_resource_at(root: &Path, rel: &Path) -> Option<Resource> {
    let abs = root.join(rel);
    if !abs.exists() {
        // Deleted paths still need identity for remove; callers handle that separately.
        return lattice_runtime::resource_for_event(rel);
    }
    let is_dir = abs.is_dir();
    if let Some(mut resource) = lattice_runtime::resource_for_event(rel) {
        if is_dir && !resource.kind.is_package() {
            resource.kind = ResourceKind::Folder;
        }
        return Some(resource);
    }
    if is_dir {
        let kind = ResourceKind::classify(rel, true);
        if kind.is_package() || kind == ResourceKind::Folder || kind == ResourceKind::File {
            return Some(Resource {
                path: rel.to_path_buf(),
                kind: if kind.is_package() {
                    kind
                } else {
                    ResourceKind::Folder
                },
            });
        }
    }
    None
}

fn entry_for_path(
    root: &Path,
    rel: &Path,
    registry: &mut NamespaceRegistry,
    known_resources: Option<&[Resource]>,
) -> Result<Option<CatalogEntry>, String> {
    let Some(resource) = classify_resource_at(root, rel) else {
        return Ok(None);
    };
    let path = path_key(&resource.path);
    let resource_id = registry
        .ensure_local_file(&path)
        .map_err(|err| err.to_string())?
        .to_string();
    let parent_id = match parent_path_of(&path) {
        Some(parent) => Some(
            registry
                .ensure_local_file(&parent)
                .map_err(|err| err.to_string())?
                .to_string(),
        ),
        None => None,
    };
    let child_count = known_resources
        .map(|resources| {
            resources
                .iter()
                .filter(|candidate| parent_path_of(&path_key(&candidate.path)).as_deref() == Some(path.as_str()))
                .count() as u32
        })
        .unwrap_or(0);
    Ok(Some(CatalogEntry {
        resource_id,
        path,
        kind: resource.kind,
        parent_id,
        child_count,
    }))
}

/// Bridge a reconciled filesystem event into a catalog delta when possible.
pub fn catalog_delta_for_workspace_event(
    root: &Path,
    event: &WorkspaceEvent,
) -> Result<Option<CatalogDelta>, String> {
    let mut registry = NamespaceRegistry::open(root).map_err(|err| err.to_string())?;
    let mut dirty = false;
    let delta = match event {
        WorkspaceEvent::RootDeleted => Some(CatalogDelta::Replace {
            entries: Vec::new(),
        }),
        WorkspaceEvent::Created { path, .. } | WorkspaceEvent::Modified { path, .. } => {
            let before = registry.entries().count();
            let entry = entry_for_path(root, path, &mut registry, None)?;
            dirty = dirty || registry.entries().count() != before;
            entry.map(|entry| CatalogDelta::Upsert {
                entries: vec![entry],
            })
        }
        WorkspaceEvent::Deleted { path } => {
            let key = path_key(path);
            match registry.remove(&key) {
                Ok(Some(resource_id)) => {
                    dirty = true;
                    Some(CatalogDelta::Remove {
                        resource_ids: vec![resource_id.to_string()],
                    })
                }
                Ok(None) => None,
                Err(err) => return Err(err.to_string()),
            }
        }
        WorkspaceEvent::Renamed { from, to, .. } => {
            let from_key = path_key(from);
            let to_key = path_key(to);
            match registry.rename(&from_key, &to_key) {
                Ok(_) => {
                    dirty = true;
                }
                Err(_) => {
                    // Source may be unregistered; still mint/ensure destination.
                    let _ = registry.ensure_local_file(&to_key);
                    dirty = true;
                }
            }
            let entry = entry_for_path(root, to, &mut registry, None)?;
            entry.map(|entry| CatalogDelta::Upsert {
                entries: vec![entry],
            })
        }
    };
    if dirty {
        registry.save().map_err(|err| err.to_string())?;
    }
    Ok(delta)
}

fn resolve_parent_path(
    registry: &NamespaceRegistry,
    parent_id: Option<&str>,
    parent_path: Option<&str>,
) -> Result<Option<String>, String> {
    match (parent_id.map(str::trim).filter(|v| !v.is_empty()), parent_path.map(str::trim).filter(|v| !v.is_empty())) {
        (None, None) => Ok(None),
        (Some(id), path_opt) => {
            let resource_id = ResourceId::from_str(id).map_err(|err| err.to_string())?;
            let resolved = registry
                .path_for_resource_id(resource_id)
                .ok_or_else(|| format!("unknown parent resource id: {id}"))?;
            if let Some(path) = path_opt {
                let normalized = path.trim_matches('/').replace('\\', "/");
                if normalized != resolved {
                    return Err(format!(
                        "parent_id {id} resolves to {resolved}, not {normalized}"
                    ));
                }
            }
            Ok(Some(resolved))
        }
        (None, Some(path)) => Ok(Some(path.trim_matches('/').replace('\\', "/"))),
    }
}

/// List direct children under a parent (by id and/or path) with cursor pagination.
pub fn list_children(
    root: String,
    parent_id: Option<String>,
    parent_path: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<ListChildrenPage, String> {
    list_children_with_runtime(
        &default_runtime(),
        root,
        parent_id,
        parent_path,
        cursor,
        limit,
    )
}

pub fn list_children_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    parent_id: Option<String>,
    parent_path: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<ListChildrenPage, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    list_children_with_session(&session, parent_id, parent_path, cursor, limit)
}

pub fn list_children_with_session(
    session: &WorkspaceSession,
    parent_id: Option<String>,
    parent_path: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<ListChildrenPage, String> {
    list_children_from_workspace(
        session.workspace(),
        parent_id.as_deref(),
        parent_path.as_deref(),
        cursor.as_deref(),
        limit,
    )
}

pub fn list_children_from_workspace(
    workspace: &Workspace,
    parent_id: Option<&str>,
    parent_path: Option<&str>,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<ListChildrenPage, String> {
    let resources = workspace.scan().map_err(|err| err.to_string())?;
    let mut registry = NamespaceRegistry::open(workspace.root()).map_err(|err| err.to_string())?;
    let before = registry.entries().count();
    let entries = catalog_entries_from_resources(&resources, &mut registry)?;
    if registry.entries().count() != before {
        registry.save().map_err(|err| err.to_string())?;
    }
    let parent = resolve_parent_path(&registry, parent_id, parent_path)?;
    let children = entries
        .into_iter()
        .filter(|entry| is_direct_child(&entry.path, parent.as_deref()))
        .collect::<Vec<_>>();
    Ok(paginate_children(children, cursor, clamp_limit(limit)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Catalog Test").unwrap();
        dir
    }

    fn entry(id: &str, path: &str, kind: ResourceKind) -> CatalogEntry {
        CatalogEntry {
            resource_id: id.into(),
            path: path.into(),
            kind,
            parent_id: parent_path_of(path).map(|_| format!("parent-of-{path}")),
            child_count: 0,
        }
    }

    #[test]
    fn parent_path_and_direct_child_helpers() {
        assert_eq!(parent_path_of("Notes/a.md").as_deref(), Some("Notes"));
        assert_eq!(parent_path_of("a.md"), None);
        assert!(is_direct_child("Notes/a.md", Some("Notes")));
        assert!(is_direct_child("a.md", None));
        assert!(!is_direct_child("Notes/nested/a.md", Some("Notes")));
    }

    #[test]
    fn paginate_children_uses_path_cursor() {
        let children = vec![
            entry("1", "a.md", ResourceKind::Page),
            entry("2", "b.md", ResourceKind::Page),
            entry("3", "c.md", ResourceKind::Page),
            entry("4", "d.md", ResourceKind::Page),
        ];
        let page = paginate_children(children.clone(), None, 2);
        assert_eq!(
            page.children
                .iter()
                .map(|e| e.path.as_str())
                .collect::<Vec<_>>(),
            ["a.md", "b.md"]
        );
        assert_eq!(page.next_cursor.as_deref(), Some("b.md"));

        let page2 = paginate_children(children, page.next_cursor.as_deref(), 2);
        assert_eq!(
            page2
                .children
                .iter()
                .map(|e| e.path.as_str())
                .collect::<Vec<_>>(),
            ["c.md", "d.md"]
        );
        assert_eq!(page2.next_cursor, None);
    }

    #[test]
    fn apply_catalog_delta_upsert_remove_replace() {
        let base = apply_catalog_delta(
            &HashMap::new(),
            &CatalogDelta::Replace {
                entries: vec![
                    entry("a", "a.md", ResourceKind::Page),
                    entry("b", "b.md", ResourceKind::Page),
                ],
            },
        );
        assert_eq!(base.len(), 2);

        let upserted = apply_catalog_delta(
            &base,
            &CatalogDelta::Upsert {
                entries: vec![entry("c", "c.md", ResourceKind::Page)],
            },
        );
        assert_eq!(upserted.len(), 3);

        let removed = apply_catalog_delta(
            &upserted,
            &CatalogDelta::Remove {
                resource_ids: vec!["b".into()],
            },
        );
        assert_eq!(removed.len(), 2);
        assert!(!removed.contains_key("b"));

        let reordered = apply_catalog_delta(
            &removed,
            &CatalogDelta::Reorder {
                parent_id: None,
                ordered_ids: vec!["c".into(), "a".into()],
            },
        );
        assert_eq!(reordered, removed);
    }

    #[test]
    fn list_children_pages_root_and_folder() {
        let dir = init_workspace();
        std::fs::create_dir_all(dir.path().join("Notes")).unwrap();
        std::fs::write(dir.path().join("root.md"), "# root\n").unwrap();
        std::fs::write(dir.path().join("Notes/a.md"), "# a\n").unwrap();
        std::fs::write(dir.path().join("Notes/b.md"), "# b\n").unwrap();
        std::fs::write(dir.path().join("Notes/c.md"), "# c\n").unwrap();

        let root = dir.path().to_string_lossy().into_owned();
        let page = list_children(root.clone(), None, None, None, Some(10)).unwrap();
        let root_paths: Vec<_> = page.children.iter().map(|e| e.path.as_str()).collect();
        assert!(root_paths.contains(&"Notes"));
        assert!(root_paths.contains(&"root.md"));
        assert!(!root_paths.iter().any(|p| p.starts_with("Notes/")));

        let notes = page
            .children
            .iter()
            .find(|entry| entry.path == "Notes")
            .expect("Notes folder");
        assert!(notes.child_count >= 3);
        assert!(!notes.resource_id.is_empty());

        let nested = list_children(
            root.clone(),
            Some(notes.resource_id.clone()),
            None,
            None,
            Some(2),
        )
        .unwrap();
        assert_eq!(nested.children.len(), 2);
        assert!(nested.next_cursor.is_some());
        assert!(nested
            .children
            .iter()
            .all(|entry| entry.parent_id.as_deref() == Some(notes.resource_id.as_str())));

        let nested2 = list_children(
            root,
            Some(notes.resource_id.clone()),
            None,
            nested.next_cursor.clone(),
            Some(2),
        )
        .unwrap();
        assert!(!nested2.children.is_empty());
        assert_eq!(nested2.next_cursor, None);
        let all_ids: std::collections::HashSet<_> = nested
            .children
            .iter()
            .chain(nested2.children.iter())
            .map(|entry| entry.resource_id.as_str())
            .collect();
        assert_eq!(all_ids.len(), 3);
    }

    #[test]
    fn catalog_delta_bridges_create_and_delete() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("note.md"), "# hi\n").unwrap();

        let created = catalog_delta_for_workspace_event(
            dir.path(),
            &WorkspaceEvent::Created {
                path: PathBuf::from("note.md"),
                revision: "rev-1".into(),
            },
        )
        .unwrap()
        .expect("created delta");
        let CatalogDelta::Upsert { entries } = created else {
            panic!("expected upsert");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "note.md");
        assert_eq!(entries[0].kind, ResourceKind::Page);
        let resource_id = entries[0].resource_id.clone();

        std::fs::remove_file(dir.path().join("note.md")).unwrap();
        let deleted = catalog_delta_for_workspace_event(
            dir.path(),
            &WorkspaceEvent::Deleted {
                path: PathBuf::from("note.md"),
            },
        )
        .unwrap()
        .expect("deleted delta");
        assert_eq!(
            deleted,
            CatalogDelta::Remove {
                resource_ids: vec![resource_id],
            }
        );
    }

    #[test]
    fn catalog_delta_rename_preserves_resource_id() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("old.md"), "# hi\n").unwrap();
        let created = catalog_delta_for_workspace_event(
            dir.path(),
            &WorkspaceEvent::Created {
                path: PathBuf::from("old.md"),
                revision: "rev-1".into(),
            },
        )
        .unwrap()
        .unwrap();
        let CatalogDelta::Upsert { entries } = created else {
            panic!("expected upsert");
        };
        let original_id = entries[0].resource_id.clone();

        std::fs::rename(dir.path().join("old.md"), dir.path().join("new.md")).unwrap();
        let renamed = catalog_delta_for_workspace_event(
            dir.path(),
            &WorkspaceEvent::Renamed {
                from: PathBuf::from("old.md"),
                to: PathBuf::from("new.md"),
                revision: "rev-2".into(),
            },
        )
        .unwrap()
        .unwrap();
        let CatalogDelta::Upsert { entries } = renamed else {
            panic!("expected upsert");
        };
        assert_eq!(entries[0].path, "new.md");
        assert_eq!(entries[0].resource_id, original_id);
    }
}
