//! Attachment staging, promotion, and orphan cleanup.
//!
//! Binaries are staged under `.lattice/staging/attachments/<operation-id>/`
//! until a record insert/update commits. Cell values store package-relative
//! `attachments/<file>` paths after promotion. Removing a path from a cell
//! drops the reference only; physical delete is reserved for verified orphan
//! cleanup so undo and shared refs cannot destroy still-needed files.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};

use uuid::Uuid;

use crate::data_app::{validate_attachment_ref, DataApp};
use crate::types::{CellValue, FieldType};
use crate::{Error, Result};

/// Workspace-relative prefix for staged attachment binaries.
pub const STAGED_ATTACHMENT_PREFIX: &str = ".lattice/staging/attachments";

/// Returns true when `path` is a workspace-relative staged attachment identity.
pub fn is_staged_attachment_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.starts_with(&format!("{STAGED_ATTACHMENT_PREFIX}/"))
        && normalized != format!("{STAGED_ATTACHMENT_PREFIX}/")
}

/// Fail-closed validation for staged attachment identities.
pub fn validate_staged_attachment_ref(table: &str, column: &str, path: &str) -> Result<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(Error::table(
            table,
            format!("column {column:?} staged attachment path must be non-empty"),
        ));
    }
    if trimmed != path {
        return Err(Error::table(
            table,
            format!(
                "column {column:?} staged attachment path {path:?} must not have leading/trailing whitespace"
            ),
        ));
    }
    if Path::new(trimmed).is_absolute() {
        return Err(Error::table(
            table,
            format!("column {column:?} staged attachment path {path:?} must be workspace-relative"),
        ));
    }
    let normalized = trimmed.replace('\\', "/");
    let prefix = format!("{STAGED_ATTACHMENT_PREFIX}/");
    if !normalized.starts_with(&prefix) {
        return Err(Error::table(
            table,
            format!(
                "column {column:?} staged attachment path {path:?} must be under {STAGED_ATTACHMENT_PREFIX}/"
            ),
        ));
    }
    let rest = &normalized[prefix.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() != 2
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == ".." || part.contains('\\'))
    {
        return Err(Error::table(
            table,
            format!("column {column:?} staged attachment path {path:?} is invalid"),
        ));
    }
    Ok(())
}

/// Copy `source_path` into a fresh staging operation directory.
///
/// Returns the workspace-relative staged path
/// (`.lattice/staging/attachments/<operation-id>/<name>`).
pub fn stage_attachment_file(workspace_root: &Path, source_path: &Path) -> Result<String> {
    if !source_path.is_file() {
        return Err(Error::invalid_package(
            source_path,
            "attachment source must be an existing file",
        ));
    }

    let operation_id = Uuid::now_v7().to_string();
    let staging_dir = workspace_root
        .join(".lattice")
        .join("staging")
        .join("attachments")
        .join(&operation_id);
    std::fs::create_dir_all(&staging_dir).map_err(|source| Error::io(&staging_dir, source))?;

    let original_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment");
    let safe_name = sanitize_attachment_filename(original_name);
    let dest = staging_dir.join(&safe_name);
    // Refuse unexpected destination shapes (sanitizer should already prevent this).
    ensure_path_within(&staging_dir, &dest)?;

    std::fs::copy(source_path, &dest).map_err(|source| Error::io(&dest, source))?;
    Ok(format!("{STAGED_ATTACHMENT_PREFIX}/{operation_id}/{safe_name}").replace('\\', "/"))
}

/// Delete one staged attachment file (and its empty operation directory).
pub fn discard_staged_attachment(workspace_root: &Path, staged_rel: &str) -> Result<()> {
    validate_staged_attachment_ref("attachments", "path", staged_rel)?;
    let absolute = resolve_staged_absolute(workspace_root, staged_rel)?;
    if absolute.is_file() {
        std::fs::remove_file(&absolute).map_err(|source| Error::io(&absolute, source))?;
    }
    if let Some(parent) = absolute.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

/// Promote a staged file into `{package}/attachments/`, returning the package-relative path.
///
/// Leaves the staged file in place; callers should
/// [`discard_staged_attachment`] after the record mutation succeeds.
pub fn promote_staged_attachment(
    workspace_root: &Path,
    app: &DataApp,
    staged_rel: &str,
) -> Result<String> {
    validate_staged_attachment_ref("attachments", "path", staged_rel)?;
    let absolute = resolve_staged_absolute(workspace_root, staged_rel)?;
    if !absolute.is_file() {
        return Err(Error::invalid_package(
            &absolute,
            "staged attachment file is missing",
        ));
    }
    app.add_attachment_file(&absolute)
}

/// Rewrite attachment cell values so staged paths become package `attachments/` refs.
///
/// Returns `(promoted_package_paths, staged_paths_to_discard_on_success)`.
/// On failure after partial promotion, newly copied package files from this call
/// are removed before the error propagates.
pub fn promote_attachment_cell_values(
    workspace_root: &Path,
    app: &DataApp,
    values: &mut BTreeMap<String, CellValue>,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut promoted = Vec::new();
    let mut staged_sources = Vec::new();
    let result = (|| -> Result<()> {
        for value in values.values_mut() {
            let CellValue::Attachment { paths } = value else {
                continue;
            };
            let mut rewritten = Vec::with_capacity(paths.len());
            for path in std::mem::take(paths) {
                if is_staged_attachment_path(&path) {
                    let package_path = promote_staged_attachment(workspace_root, app, &path)?;
                    promoted.push(package_path.clone());
                    staged_sources.push(path);
                    rewritten.push(package_path);
                } else {
                    validate_attachment_ref("attachments", "path", &path)?;
                    rewritten.push(path);
                }
            }
            *paths = rewritten;
        }
        Ok(())
    })();
    if let Err(err) = result {
        let _ = cleanup_unreferenced_attachments(app, &promoted);
        return Err(err);
    }
    Ok((promoted, staged_sources))
}

/// Collect every package-relative attachment path referenced by attachment cells.
pub fn collect_attachment_refs(app: &DataApp) -> Result<HashSet<String>> {
    let mut refs = HashSet::new();
    for table in app.list_tables()? {
        let columns = app.columns(&table)?;
        let attachment_columns: Vec<String> = columns
            .iter()
            .filter(|column| column.field_type == FieldType::Attachment)
            .map(|column| column.name.clone())
            .collect();
        if attachment_columns.is_empty() {
            continue;
        }
        let total = app.count_rows(&table)?;
        let rows = app.list_rows(&table, total, 0)?;
        for row in rows {
            for column in &attachment_columns {
                if let Some(CellValue::Attachment { paths }) = row.values.get(column) {
                    for path in paths {
                        refs.insert(path.clone());
                    }
                }
            }
        }
    }
    Ok(refs)
}

/// List files under `{package}/attachments/` that are not referenced by any cell.
pub fn list_orphan_attachments(app: &DataApp) -> Result<Vec<String>> {
    let refs = collect_attachment_refs(app)?;
    let attachments_dir = app.attachments_dir();
    if !attachments_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut orphans = Vec::new();
    for entry in
        std::fs::read_dir(&attachments_dir).map_err(|source| Error::io(&attachments_dir, source))?
    {
        let entry = entry.map_err(|source| Error::io(&attachments_dir, source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io(entry.path(), source))?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
            continue;
        }
        let package_rel = format!("attachments/{name}");
        if !refs.contains(&package_rel) {
            orphans.push(package_rel);
        }
    }
    orphans.sort();
    Ok(orphans)
}

/// Delete verified orphans under `{package}/attachments/`.
///
/// Returns the package-relative paths that were removed.
pub fn cleanup_orphan_attachments(app: &DataApp) -> Result<Vec<String>> {
    let orphans = list_orphan_attachments(app)?;
    cleanup_unreferenced_attachments(app, &orphans)
}

/// Delete `candidates` only when they are not referenced by any attachment cell.
pub fn cleanup_unreferenced_attachments(
    app: &DataApp,
    candidates: &[String],
) -> Result<Vec<String>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let refs = collect_attachment_refs(app)?;
    let mut deleted = Vec::new();
    let mut seen = BTreeSet::new();
    for path in candidates {
        if !seen.insert(path.clone()) {
            continue;
        }
        validate_attachment_ref("attachments", "path", path)?;
        if refs.contains(path) {
            continue;
        }
        app.remove_attachment_file(path)?;
        deleted.push(path.clone());
    }
    Ok(deleted)
}

/// Collect attachment package paths present in command cell values.
pub fn attachment_paths_in_values(values: &BTreeMap<String, CellValue>) -> Vec<String> {
    let mut paths = Vec::new();
    for value in values.values() {
        if let CellValue::Attachment { paths: cell_paths } = value {
            paths.extend(cell_paths.iter().cloned());
        }
    }
    paths
}

fn resolve_staged_absolute(workspace_root: &Path, staged_rel: &str) -> Result<PathBuf> {
    let normalized = staged_rel.replace('\\', "/");
    let prefix = format!("{STAGED_ATTACHMENT_PREFIX}/");
    let rest = normalized
        .strip_prefix(&prefix)
        .ok_or_else(|| Error::invalid_package(staged_rel, "staged attachment path is invalid"))?;
    let mut parts = rest.split('/');
    let operation_id = parts
        .next()
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .ok_or_else(|| Error::invalid_package(staged_rel, "staged attachment path is invalid"))?;
    let file_name = parts
        .next()
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .ok_or_else(|| Error::invalid_package(staged_rel, "staged attachment path is invalid"))?;
    if parts.next().is_some() {
        return Err(Error::invalid_package(
            staged_rel,
            "staged attachment path is invalid",
        ));
    }
    let staging_root = workspace_root
        .join(".lattice")
        .join("staging")
        .join("attachments");
    let absolute = staging_root.join(operation_id).join(file_name);
    ensure_path_within(&staging_root, &absolute)?;
    Ok(absolute)
}

fn ensure_path_within(root: &Path, candidate: &Path) -> Result<()> {
    // Component walk rejects `..` even when the path does not exist yet.
    for component in candidate.components() {
        if matches!(component, Component::ParentDir) {
            return Err(Error::invalid_package(
                candidate,
                "attachment path escapes staging root",
            ));
        }
    }
    if !root.exists() {
        return Ok(());
    }
    let root_canon = std::fs::canonicalize(root).map_err(|source| Error::io(root, source))?;
    let candidate_canon = if candidate.exists() {
        std::fs::canonicalize(candidate).map_err(|source| Error::io(candidate, source))?
    } else if let Some(parent) = candidate.parent().filter(|parent| parent.exists()) {
        let parent_canon =
            std::fs::canonicalize(parent).map_err(|source| Error::io(parent, source))?;
        let name = candidate
            .file_name()
            .ok_or_else(|| Error::invalid_package(candidate, "attachment path has no file name"))?;
        parent_canon.join(name)
    } else {
        return Ok(());
    };
    if !candidate_canon.starts_with(&root_canon) {
        return Err(Error::invalid_package(
            candidate,
            "attachment path escapes staging root",
        ));
    }
    Ok(())
}

fn sanitize_attachment_filename(name: &str) -> String {
    let trimmed = name.trim();
    let base = if trimmed.is_empty() {
        "attachment"
    } else {
        trimmed
    };
    let sanitized: String = base
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '_',
        })
        .collect();
    let collapsed = sanitized.trim_matches('_').trim_matches('.');
    if collapsed.is_empty() || collapsed == "." || collapsed == ".." {
        "attachment".to_string()
    } else {
        collapsed.to_string()
    }
}
