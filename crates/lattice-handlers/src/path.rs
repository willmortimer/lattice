use std::path::{Path, PathBuf};

/// Canonicalize `root` and a `rel_path` candidate beneath it, rejecting `..`
/// traversal and absolute-path escapes (including through symlinks) by
/// requiring the resolved candidate to remain under the canonical root.
pub fn resolve_within_root(root: &str, rel_path: &str) -> Result<(PathBuf, PathBuf), String> {
    let canonical_root = PathBuf::from(root)
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))?;

    let candidate = canonical_root.join(rel_path);
    let canonical_candidate = candidate
        .canonicalize()
        .map_err(|err| format!("cannot resolve {rel_path:?}: {err}"))?;

    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }

    Ok((canonical_root, canonical_candidate))
}

/// Reject workspace-relative paths that escape the root via `..` or absolute
/// prefixes. Used when the target path may not exist yet (folder create).
pub fn validate_workspace_relative(rel_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel_path);
    if rel_path.trim().is_empty() {
        return Err("path must be non-empty".to_string());
    }
    if path.is_absolute() {
        return Err(format!("{rel_path:?} must be workspace-relative"));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }
    Ok(path.to_path_buf())
}

/// Join a validated workspace-relative path under a canonical root.
pub fn join_within_root(root: &str, rel_path: &str) -> Result<(PathBuf, PathBuf), String> {
    let canonical_root = PathBuf::from(root)
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))?;
    let relative = validate_workspace_relative(rel_path)?;
    let candidate = canonical_root.join(&relative);
    if !candidate.starts_with(&canonical_root) {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }
    Ok((canonical_root, relative))
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

    #[test]
    fn resolve_within_root_reads_existing_file() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let (_, candidate) = resolve_within_root(&root, "Notes.md").unwrap();
        assert_eq!(
            std::fs::read_to_string(candidate).unwrap(),
            "# Hi\n"
        );
    }

    #[test]
    fn resolve_within_root_rejects_relative_traversal() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        Workspace::init(&ws, "Inner").unwrap();

        let result = resolve_within_root(
            &ws.to_string_lossy(),
            "../secret.txt",
        );
        assert!(result.is_err());
    }
}
