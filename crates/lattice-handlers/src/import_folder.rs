//! Copy a folder into an already-open workspace (Drive-style import).
//!
//! Does not provision a workspace or a cloud row. New files are ordinary
//! writes; the existing watcher/catalog picks them up.

use std::fs;
use std::path::{Component, Path, PathBuf};

use lattice_core::{Workspace, OPERATIONAL_DIR, WORKSPACE_MANIFEST_FILENAME};
use lattice_storage::atomic_write_file;
use serde::Serialize;

use crate::path::{join_within_root, validate_workspace_relative};

const CONFLICT_REASON: &str = "destination exists with different content";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderImportSkippedEntry {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderImportResult {
    pub dest_dir: String,
    pub copied_count: u64,
    pub skipped: Vec<FolderImportSkippedEntry>,
}

/// Copy `source_dir` into `root/dest_dir`.
///
/// When `dest_dir` is omitted or blank, the destination name is the source
/// folder's last path component. Existing destination files with different
/// bytes are skipped (same conflict policy as encrypted restore). Source
/// `lattice.yaml` and `.lattice/` are not copied.
pub fn import_folder_into_workspace(
    root: &str,
    source_dir: &str,
    dest_dir: Option<&str>,
) -> Result<FolderImportResult, String> {
    let source_dir = require_non_empty(source_dir, "source folder")?;
    let workspace = Workspace::open(Path::new(root)).map_err(|err| err.to_string())?;
    let canonical_root = workspace
        .root()
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))?;

    let canonical_source = PathBuf::from(source_dir)
        .canonicalize()
        .map_err(|err| format!("cannot resolve source folder {source_dir:?}: {err}"))?;
    if !canonical_source.is_dir() {
        return Err(format!("{source_dir:?} is not a directory"));
    }
    if canonical_root == canonical_source || canonical_root.starts_with(&canonical_source) {
        return Err("cannot import a folder that contains the open workspace".to_string());
    }

    let dest_rel = resolve_dest_dir(&canonical_source, dest_dir)?;
    let dest_rel_str = posix_rel(&dest_rel);
    let (_, dest_validated) = join_within_root(root, &dest_rel_str)?;
    let dest_abs = canonical_root.join(&dest_validated);
    if !dest_abs.starts_with(&canonical_root) {
        return Err(format!("{dest_rel_str:?} escapes the workspace root"));
    }
    if dest_abs.exists() && !dest_abs.is_dir() {
        return Err(format!(
            "destination {} exists and is not a directory",
            dest_abs.display()
        ));
    }
    fs::create_dir_all(&dest_abs)
        .map_err(|err| format!("create destination {}: {err}", dest_abs.display()))?;

    let mut copied_count = 0u64;
    let mut skipped = Vec::new();
    copy_tree(
        &canonical_root,
        &canonical_source,
        &canonical_source,
        &dest_validated,
        &mut copied_count,
        &mut skipped,
    )?;

    Ok(FolderImportResult {
        dest_dir: dest_rel_str,
        copied_count,
        skipped,
    })
}

fn resolve_dest_dir(canonical_source: &Path, dest_dir: Option<&str>) -> Result<PathBuf, String> {
    let trimmed = dest_dir.map(str::trim).filter(|value| !value.is_empty());
    match trimmed {
        Some(dest) => validate_workspace_relative(dest),
        None => {
            let name = canonical_source
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "source folder name is required".to_string())?;
            validate_workspace_relative(name)
        }
    }
}

fn copy_tree(
    canonical_root: &Path,
    source_root: &Path,
    current: &Path,
    dest_rel: &Path,
    copied_count: &mut u64,
    skipped: &mut Vec<FolderImportSkippedEntry>,
) -> Result<(), String> {
    let entries =
        fs::read_dir(current).map_err(|err| format!("read source {}: {err}", current.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read source entry: {err}"))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("stat {}: {err}", entry.path().display()))?;
        // Do not follow symlinks: a link could point outside the picked folder.
        if file_type.is_symlink() {
            continue;
        }

        let source_path = entry.path();
        let source_rel = source_path
            .strip_prefix(source_root)
            .map_err(|_| format!("{} is outside the source folder", source_path.display()))?;
        if should_skip_source(source_rel) {
            continue;
        }

        let dest_file_rel = dest_rel.join(source_rel);
        let dest_file_str = posix_rel(&dest_file_rel);
        let dest_validated = validate_workspace_relative(&dest_file_str)?;
        let dest_abs = canonical_root.join(&dest_validated);
        if !dest_abs.starts_with(canonical_root) {
            return Err(format!("{dest_file_str:?} escapes the workspace root"));
        }

        if file_type.is_dir() {
            fs::create_dir_all(&dest_abs)
                .map_err(|err| format!("create directory {}: {err}", dest_abs.display()))?;
            copy_tree(
                canonical_root,
                source_root,
                &source_path,
                dest_rel,
                copied_count,
                skipped,
            )?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let bytes = fs::read(&source_path)
            .map_err(|err| format!("read {}: {err}", source_path.display()))?;
        apply_import_file(&dest_abs, &dest_file_str, &bytes, copied_count, skipped)?;
    }
    Ok(())
}

fn apply_import_file(
    dest: &Path,
    rel: &str,
    bytes: &[u8],
    copied_count: &mut u64,
    skipped: &mut Vec<FolderImportSkippedEntry>,
) -> Result<(), String> {
    if dest.exists() {
        if dest.is_dir() {
            return Err(format!(
                "destination {} exists and is a directory",
                dest.display()
            ));
        }
        let existing =
            fs::read(dest).map_err(|err| format!("read existing {}: {err}", dest.display()))?;
        if existing != bytes {
            skipped.push(FolderImportSkippedEntry {
                path: rel.to_string(),
                reason: CONFLICT_REASON.into(),
            });
            return Ok(());
        }
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent {}: {err}", parent.display()))?;
    }
    atomic_write_file(dest, bytes).map_err(|err| err.to_string())?;
    *copied_count += 1;
    Ok(())
}

/// Skip the source workspace manifest and operational directory only.
fn should_skip_source(rel: &Path) -> bool {
    let mut components = rel.components();
    match components.next() {
        Some(Component::Normal(name)) if name == OPERATIONAL_DIR => true,
        Some(Component::Normal(name))
            if name == WORKSPACE_MANIFEST_FILENAME && components.next().is_none() =>
        {
            true
        }
        _ => false,
    }
}

fn posix_rel(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn require_non_empty<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn copies_nested_files_into_source_named_dest() {
        let workspace = init_workspace();
        let source = tempfile::tempdir().unwrap();
        let source_root = source.path().join("Notes");
        write(&source_root.join("hello.md"), b"# Hi\n");
        write(&source_root.join("nested/deep.txt"), b"deep\n");

        let root = workspace.path().to_string_lossy().into_owned();
        let result =
            import_folder_into_workspace(&root, &source_root.to_string_lossy(), None).unwrap();

        assert_eq!(result.dest_dir, "Notes");
        assert_eq!(result.copied_count, 2);
        assert!(result.skipped.is_empty());
        assert_eq!(
            fs::read_to_string(workspace.path().join("Notes/hello.md")).unwrap(),
            "# Hi\n"
        );
        assert_eq!(
            fs::read_to_string(workspace.path().join("Notes/nested/deep.txt")).unwrap(),
            "deep\n"
        );
    }

    #[test]
    fn skips_source_manifest_and_operational_dir() {
        let workspace = init_workspace();
        let original_manifest =
            fs::read_to_string(workspace.path().join(WORKSPACE_MANIFEST_FILENAME)).unwrap();

        let source = tempfile::tempdir().unwrap();
        let source_root = source.path().join("Incoming");
        write(&source_root.join("keep.md"), b"keep\n");
        write(
            &source_root.join(WORKSPACE_MANIFEST_FILENAME),
            b"id: should-not-clobber\n",
        );
        write(
            &source_root.join(OPERATIONAL_DIR).join("index.bin"),
            b"secret",
        );

        let root = workspace.path().to_string_lossy().into_owned();
        let result =
            import_folder_into_workspace(&root, &source_root.to_string_lossy(), Some("Incoming"))
                .unwrap();

        assert_eq!(result.copied_count, 1);
        assert_eq!(
            fs::read_to_string(workspace.path().join("Incoming/keep.md")).unwrap(),
            "keep\n"
        );
        assert!(!workspace
            .path()
            .join("Incoming")
            .join(WORKSPACE_MANIFEST_FILENAME)
            .exists());
        assert!(!workspace
            .path()
            .join("Incoming")
            .join(OPERATIONAL_DIR)
            .exists());
        assert_eq!(
            fs::read_to_string(workspace.path().join(WORKSPACE_MANIFEST_FILENAME)).unwrap(),
            original_manifest
        );
    }

    #[test]
    fn skips_conflicting_different_bytes() {
        let workspace = init_workspace();
        write(&workspace.path().join("Notes/hello.md"), b"local\n");

        let source = tempfile::tempdir().unwrap();
        let source_root = source.path().join("Notes");
        write(&source_root.join("hello.md"), b"incoming\n");
        write(&source_root.join("new.md"), b"fresh\n");

        let root = workspace.path().to_string_lossy().into_owned();
        let result =
            import_folder_into_workspace(&root, &source_root.to_string_lossy(), Some("Notes"))
                .unwrap();

        assert_eq!(result.copied_count, 1);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].path, "Notes/hello.md");
        assert_eq!(result.skipped[0].reason, CONFLICT_REASON);
        assert_eq!(
            fs::read_to_string(workspace.path().join("Notes/hello.md")).unwrap(),
            "local\n"
        );
        assert_eq!(
            fs::read_to_string(workspace.path().join("Notes/new.md")).unwrap(),
            "fresh\n"
        );
    }

    #[test]
    fn rejects_dest_that_escapes_workspace_root() {
        let workspace = init_workspace();
        let source = tempfile::tempdir().unwrap();
        let source_root = source.path().join("Notes");
        write(&source_root.join("hello.md"), b"# Hi\n");

        let root = workspace.path().to_string_lossy().into_owned();
        let err =
            import_folder_into_workspace(&root, &source_root.to_string_lossy(), Some("../escape"))
                .unwrap_err();
        assert!(
            err.contains("escapes the workspace root"),
            "unexpected error: {err}"
        );
        assert!(!workspace.path().parent().unwrap().join("escape").exists());
    }

    #[test]
    fn copies_into_explicit_dest_dir() {
        let workspace = init_workspace();
        fs::create_dir_all(workspace.path().join("Projects")).unwrap();

        let source = tempfile::tempdir().unwrap();
        let source_root = source.path().join("Notes");
        write(&source_root.join("hello.md"), b"# Hi\n");

        let root = workspace.path().to_string_lossy().into_owned();
        let result = import_folder_into_workspace(
            &root,
            &source_root.to_string_lossy(),
            Some("Projects/Notes"),
        )
        .unwrap();

        assert_eq!(result.dest_dir, "Projects/Notes");
        assert_eq!(result.copied_count, 1);
        assert_eq!(
            fs::read_to_string(workspace.path().join("Projects/Notes/hello.md")).unwrap(),
            "# Hi\n"
        );
    }
}
