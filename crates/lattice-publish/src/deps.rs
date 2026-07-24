//! Dependency closure for static export: collect, copy, and report local refs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// Kind of dependency referenced by export content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    PageAsset,
    ChartSpec,
    Attachment,
    LocalFile,
}

impl DependencyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PageAsset => "page-asset",
            Self::ChartSpec => "chart-spec",
            Self::Attachment => "attachment",
            Self::LocalFile => "local-file",
        }
    }
}

/// A dependency that was copied into the export directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedDependency {
    /// Path as declared in source content.
    pub declared: String,
    /// Path relative to the export `out_dir`.
    pub dest: String,
    pub kind: &'static str,
}

/// A dependency that could not be copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub declared: String,
    pub reason: String,
    /// When true, export fails after collection.
    pub required: bool,
    pub kind: &'static str,
}

/// Result of materializing the dependency closure into `out_dir`.
#[derive(Debug, Default, Clone)]
pub struct DependencyClosure {
    pub copied: Vec<CopiedDependency>,
    pub missing: Vec<MissingDependency>,
    /// Map from declared path → export-relative destination for link rewrites.
    pub rewrites: BTreeMap<String, String>,
}

struct PendingDep {
    declared: String,
    kind: DependencyKind,
    required: bool,
    /// Absolute source path when the declared ref resolves under the workspace.
    absolute: Option<PathBuf>,
    /// Workspace-relative path used as the stable export key.
    workspace_relative: Option<String>,
    reason_if_unresolved: Option<String>,
}

/// Collects local file dependencies and copies permitted ones into `out_dir/deps/…`.
pub struct DependencyCollector {
    canonical_root: PathBuf,
    base_dir: PathBuf,
    pending: Vec<PendingDep>,
    seen: BTreeSet<String>,
}

impl DependencyCollector {
    /// `base_dir` is used to resolve relative declared paths (e.g. the page's parent).
    pub fn new(workspace_root: &Path, base_dir: &Path) -> Result<Self> {
        let canonical_root = workspace_root
            .canonicalize()
            .map_err(|source| Error::io(workspace_root, source))?;
        let base_dir = if base_dir.exists() {
            base_dir
                .canonicalize()
                .map_err(|source| Error::io(base_dir, source))?
        } else {
            base_dir.to_path_buf()
        };
        Ok(Self {
            canonical_root,
            base_dir,
            pending: Vec::new(),
            seen: BTreeSet::new(),
        })
    }

    /// Record a declared local path reference.
    pub fn add(&mut self, declared: &str, kind: DependencyKind, required: bool) {
        let declared = declared.trim();
        if declared.is_empty() || is_external_ref(declared) {
            return;
        }
        let key = format!("{}::{declared}", kind.as_str());
        if !self.seen.insert(key) {
            return;
        }

        match resolve_declared(&self.canonical_root, &self.base_dir, declared) {
            Ok(resolved) => self.pending.push(PendingDep {
                declared: declared.to_string(),
                kind,
                required,
                absolute: Some(resolved.absolute),
                workspace_relative: Some(resolved.workspace_relative),
                reason_if_unresolved: None,
            }),
            Err(reason) => self.pending.push(PendingDep {
                declared: declared.to_string(),
                kind,
                required,
                absolute: None,
                workspace_relative: None,
                reason_if_unresolved: Some(reason),
            }),
        }
    }

    /// Note a dependency that is intentionally not snapshotted (warn-only).
    pub fn add_unsupported(
        &mut self,
        declared: &str,
        kind: DependencyKind,
        reason: impl Into<String>,
    ) {
        let declared = declared.trim();
        if declared.is_empty() {
            return;
        }
        let key = format!("unsupported::{}::{declared}", kind.as_str());
        if !self.seen.insert(key) {
            return;
        }
        self.pending.push(PendingDep {
            declared: declared.to_string(),
            kind,
            required: false,
            absolute: None,
            workspace_relative: None,
            reason_if_unresolved: Some(reason.into()),
        });
    }

    /// Copy permitted files into `out_dir` and return the closure report.
    ///
    /// Fails when any **required** dependency is missing or disallowed.
    pub fn materialize(self, out_dir: &Path) -> Result<DependencyClosure> {
        let mut closure = DependencyClosure::default();
        let mut required_failures: Vec<String> = Vec::new();

        for dep in self.pending {
            let kind = dep.kind.as_str();
            if let Some(reason) = dep.reason_if_unresolved {
                closure.missing.push(MissingDependency {
                    declared: dep.declared.clone(),
                    reason,
                    required: dep.required,
                    kind,
                });
                if dep.required {
                    required_failures.push(dep.declared);
                }
                continue;
            }

            let absolute = match dep.absolute {
                Some(path) => path,
                None => continue,
            };
            let workspace_relative = match dep.workspace_relative {
                Some(rel) => rel,
                None => continue,
            };

            if !absolute.is_file() {
                let reason = if absolute.exists() {
                    "not a file".to_string()
                } else {
                    "not found".to_string()
                };
                closure.missing.push(MissingDependency {
                    declared: dep.declared.clone(),
                    reason,
                    required: dep.required,
                    kind,
                });
                if dep.required {
                    required_failures.push(dep.declared);
                }
                continue;
            }

            let dest_rel = export_dest_for(&workspace_relative);
            let dest_abs = out_dir.join(&dest_rel);
            if let Some(parent) = dest_abs.parent() {
                std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
            }
            std::fs::copy(&absolute, &dest_abs).map_err(|source| Error::io(&dest_abs, source))?;

            closure
                .rewrites
                .insert(dep.declared.clone(), dest_rel.clone());
            // Also rewrite by workspace-relative form when it differs.
            if workspace_relative != dep.declared {
                closure
                    .rewrites
                    .entry(workspace_relative.clone())
                    .or_insert_with(|| dest_rel.clone());
            }
            closure.copied.push(CopiedDependency {
                declared: dep.declared,
                dest: dest_rel,
                kind,
            });
        }

        if !required_failures.is_empty() {
            return Err(Error::message(format!(
                "required export dependencies missing or disallowed: {}",
                required_failures.join(", ")
            )));
        }

        Ok(closure)
    }
}

struct ResolvedPath {
    absolute: PathBuf,
    workspace_relative: String,
}

fn resolve_declared(
    canonical_root: &Path,
    base_dir: &Path,
    declared: &str,
) -> std::result::Result<ResolvedPath, String> {
    if Path::new(declared).is_absolute() {
        return Err("absolute paths are not allowed".into());
    }

    let candidate = base_dir.join(declared);

    let absolute = match candidate.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            // Fall back to lexical resolution for clearer missing-file reporting.
            let lexical = normalize_lexical(&candidate);
            if !lexical.starts_with(canonical_root) {
                return Err("escapes workspace root".into());
            }
            return Ok(ResolvedPath {
                workspace_relative: path_relative_to(canonical_root, &lexical)?,
                absolute: lexical,
            });
        }
    };

    if !absolute.starts_with(canonical_root) {
        return Err("escapes workspace root".into());
    }

    Ok(ResolvedPath {
        workspace_relative: path_relative_to(canonical_root, &absolute)?,
        absolute,
    })
}

fn path_relative_to(root: &Path, absolute: &Path) -> std::result::Result<String, String> {
    absolute
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "escapes workspace root".into())
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn export_dest_for(workspace_relative: &str) -> String {
    let trimmed = workspace_relative.trim_start_matches("./");
    format!("deps/{trimmed}")
}

/// True for URLs, anchors, and other non-local references.
pub fn is_external_ref(href: &str) -> bool {
    let trimmed = href.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("data:")
        || lower.starts_with("javascript:")
        || trimmed.contains("://")
}

/// Scan frozen table cell values for package-relative attachment paths.
pub fn attachment_paths_in_table(table: &crate::snapshot::TableSnapshot) -> Vec<String> {
    let mut out = Vec::new();
    for row in &table.rows {
        for cell in row {
            collect_attachment_strings(cell, &mut out);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn collect_attachment_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) if s.starts_with("attachments/") => {
            out.push(s.clone());
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_attachment_strings(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_attachment_strings(item, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn copies_local_file_under_deps() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/diagram.png"), b"png").unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();

        let mut collector = DependencyCollector::new(&root, &root).unwrap();
        collector.add("assets/diagram.png", DependencyKind::PageAsset, true);
        let closure = collector.materialize(&out).unwrap();

        assert_eq!(closure.copied.len(), 1);
        assert_eq!(closure.copied[0].dest, "deps/assets/diagram.png");
        assert!(out.join("deps/assets/diagram.png").is_file());
        assert_eq!(
            closure
                .rewrites
                .get("assets/diagram.png")
                .map(String::as_str),
            Some("deps/assets/diagram.png")
        );
    }

    #[test]
    fn required_missing_fails() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();

        let mut collector = DependencyCollector::new(&root, &root).unwrap();
        collector.add("missing.vl.json", DependencyKind::ChartSpec, true);
        let err = collector.materialize(&out).unwrap_err();
        assert!(err.to_string().contains("missing.vl.json"));
    }

    #[test]
    fn optional_missing_is_listed() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();

        let mut collector = DependencyCollector::new(&root, &root).unwrap();
        collector.add("gone.png", DependencyKind::PageAsset, false);
        let closure = collector.materialize(&out).unwrap();
        assert!(closure.copied.is_empty());
        assert_eq!(closure.missing.len(), 1);
        assert!(!closure.missing[0].required);
    }
}
