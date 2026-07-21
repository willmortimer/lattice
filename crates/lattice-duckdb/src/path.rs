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

/// Resolve a path that may include DuckDB glob metacharacters (`*`, `?`, `[`).
///
/// The longest non-glob path prefix must exist under `root`; remaining segments
/// (including globs) are appended lexically so `read_parquet('facts/**/*.parquet')`
/// stays inside the workspace allowlist.
pub fn resolve_glob_under_root(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root = canonicalize_dir(root)?;
    let absolute = absolutize(&root, candidate);

    let mut prefix = PathBuf::new();
    let mut glob_tail: Vec<std::ffi::OsString> = Vec::new();
    let mut hit_glob = false;
    for component in absolute.components() {
        let text = component.as_os_str().to_string_lossy();
        if !hit_glob && is_glob_segment(&text) {
            hit_glob = true;
        }
        if hit_glob {
            glob_tail.push(component.as_os_str().to_owned());
        } else {
            prefix.push(component);
        }
    }

    if !hit_glob {
        return resolve_under_root(&root, candidate);
    }

    let prefix = if prefix.as_os_str().is_empty() {
        root.clone()
    } else {
        prefix
            .canonicalize()
            .map_err(|source| Error::io(&prefix, source))?
    };
    if !prefix.starts_with(&root) {
        return Err(Error::path_not_allowed(prefix, root));
    }

    let mut resolved = prefix;
    for part in glob_tail {
        resolved.push(part);
    }
    Ok(resolved)
}

fn is_glob_segment(segment: &str) -> bool {
    segment.contains('*') || segment.contains('?') || segment.contains('[')
}

/// Escape a string for use inside a DuckDB single-quoted SQL literal.
pub fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Convert a filesystem path to a DuckDB-friendly forward-slash string.
pub fn path_to_sql(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Absolutize the first string-literal argument of `read_parquet` / `read_csv_auto`
/// calls so paths pass DuckDB `allowed_directories` regardless of process CWD.
///
/// Demo chart SQL uses workspace-relative globs like
/// `Data/Events.dataset/facts/**/*.parquet`. With `enable_external_access=false`,
/// those resolve against CWD (often the app bundle) and fail the allowlist.
pub fn rewrite_read_paths_under_root(sql: &str, root: &Path) -> Result<String> {
    const FNS: &[&str] = &["read_parquet", "read_csv_auto"];
    let lower = sql.to_ascii_lowercase();
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 64);
    let mut i = 0usize;

    while i < sql.len() {
        let mut matched: Option<&str> = None;
        for &name in FNS {
            if lower[i..].starts_with(name) {
                let boundary_ok = i == 0 || {
                    let prev = bytes[i - 1];
                    !(prev.is_ascii_alphanumeric() || prev == b'_')
                };
                if boundary_ok {
                    matched = Some(name);
                    break;
                }
            }
        }

        let Some(name) = matched else {
            let ch = sql[i..].chars().next().expect("index in bounds");
            out.push(ch);
            i += ch.len_utf8();
            continue;
        };

        out.push_str(&sql[i..i + name.len()]);
        i += name.len();

        while i < sql.len() && bytes[i].is_ascii_whitespace() {
            out.push(char::from(bytes[i]));
            i += 1;
        }
        if i >= sql.len() || bytes[i] != b'(' {
            continue;
        }
        out.push('(');
        i += 1;
        while i < sql.len() && bytes[i].is_ascii_whitespace() {
            out.push(char::from(bytes[i]));
            i += 1;
        }
        if i >= sql.len() || bytes[i] != b'\'' {
            continue;
        }

        i += 1; // opening quote
        let lit_start = i;
        while i < sql.len() {
            if bytes[i] == b'\'' {
                if i + 1 < sql.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                break;
            }
            i += 1;
        }
        let raw = sql[lit_start..i].replace("''", "'");
        if i < sql.len() && bytes[i] == b'\'' {
            i += 1; // closing quote
        }
        let absolute = resolve_glob_under_root(root, Path::new(&raw))?;
        out.push_str(&sql_string_literal(&path_to_sql(&absolute)));
    }

    Ok(out)
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

    #[test]
    fn rewrite_read_paths_absolutizes_relative_parquet_glob() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        let facts = root.join("Data/Events.dataset/facts");
        fs::create_dir_all(&facts).unwrap();
        fs::write(facts.join("signups.parquet"), b"parquet").unwrap();

        let sql = "SELECT region FROM read_parquet('Data/Events.dataset/facts/**/*.parquet', hive_partitioning = true)";
        let rewritten = rewrite_read_paths_under_root(sql, &root).unwrap();
        let expected_prefix = path_to_sql(&root.canonicalize().unwrap());
        assert!(
            rewritten.contains(&format!(
                "read_parquet('{expected_prefix}/Data/Events.dataset/facts/**/*.parquet'"
            )),
            "rewritten={rewritten}"
        );
        assert!(!rewritten.contains("read_parquet('Data/"), "relative path should be gone: {rewritten}");
    }
}
