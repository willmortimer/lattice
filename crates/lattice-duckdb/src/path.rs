use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::Result;

/// Canonicalize `root` and ensure an existing `candidate` resolves under it.
///
/// Relative candidates are joined to `root`. Symlinks are resolved via
/// `canonicalize`, so links that escape the workspace are rejected.
pub fn resolve_under_root(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root = canonicalize_dir(root)?;
    let absolute = absolutize(&root, candidate);
    let resolved = absolute
        .canonicalize()
        .map_err(|source| Error::io(&absolute, source))?;

    if !resolved.starts_with(&root) {
        return Err(Error::path_not_allowed(resolved, root));
    }
    Ok(resolved)
}

/// Resolve a path that may not exist yet (e.g. a new `.duckdb` file).
///
/// The parent directory must exist and stay under `root`; the final component
/// is joined lexically after the parent is canonicalized.
pub fn resolve_under_root_for_create(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root = canonicalize_dir(root)?;
    let absolute = absolutize(&root, candidate);
    if absolute.exists() {
        return resolve_under_root(&root, &absolute);
    }

    let parent = absolute
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(&root);
    let parent = parent
        .canonicalize()
        .map_err(|source| Error::io(parent, source))?;
    if !parent.starts_with(&root) {
        return Err(Error::path_not_allowed(parent, root));
    }

    let Some(name) = absolute.file_name() else {
        return Err(Error::message("path is missing a file name"));
    };
    let resolved = parent.join(name);
    if !resolved.starts_with(&root) {
        return Err(Error::path_not_allowed(resolved, root));
    }
    Ok(resolved)
}

fn canonicalize_dir(root: &Path) -> Result<PathBuf> {
    root.canonicalize()
        .map_err(|source| Error::io(root, source))
}

fn absolutize(root: &Path, candidate: &Path) -> PathBuf {
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    }
}

/// Escape a string for use inside a DuckDB single-quoted SQL literal.
pub fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_path_outside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("secret.csv");
        fs::write(&outside, "a\n1\n").unwrap();

        let err = resolve_under_root(&root, &outside).unwrap_err().to_string();
        assert!(err.contains("outside workspace root"), "{err}");
    }

    #[test]
    fn accepts_relative_path_under_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        fs::create_dir_all(root.join("facts")).unwrap();
        let csv = root.join("facts/sample.csv");
        fs::write(&csv, "a\n1\n").unwrap();

        let resolved = resolve_under_root(&root, Path::new("facts/sample.csv")).unwrap();
        assert_eq!(resolved, csv.canonicalize().unwrap());
    }

    #[test]
    fn sql_string_literal_escapes_quotes() {
        assert_eq!(sql_string_literal("a'b"), "'a''b'");
    }
}
