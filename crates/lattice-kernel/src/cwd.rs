//! Path capability gate: kernel cwd must stay under the workspace root.

use std::path::{Path, PathBuf};

use crate::error::KernelError;

/// Resolve `cwd` and ensure it is a directory contained by `workspace_root`.
///
/// Symlinks are resolved via `canonicalize`. Relative `cwd` is joined to the
/// workspace root before resolution.
pub fn resolve_cwd_under_workspace(
    workspace_root: &Path,
    cwd: &Path,
) -> Result<PathBuf, KernelError> {
    let root = workspace_root.canonicalize().map_err(|_| KernelError::CwdNotAllowed {
        cwd: cwd.to_path_buf(),
        workspace_root: workspace_root.to_path_buf(),
    })?;

    let absolute = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        root.join(cwd)
    };

    let resolved = absolute.canonicalize().map_err(|_| KernelError::CwdNotAllowed {
        cwd: absolute.clone(),
        workspace_root: root.clone(),
    })?;

    if !resolved.is_dir() {
        return Err(KernelError::CwdNotAllowed {
            cwd: resolved,
            workspace_root: root,
        });
    }

    if !resolved.starts_with(&root) {
        return Err(KernelError::CwdNotAllowed {
            cwd: resolved,
            workspace_root: root,
        });
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn accepts_workspace_root_itself() {
        let dir = tempfile::tempdir().expect("tempdir");
        let resolved = resolve_cwd_under_workspace(dir.path(), dir.path()).expect("allow");
        assert_eq!(resolved, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn accepts_relative_subdir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("notebooks");
        fs::create_dir(&nested).expect("mkdir");
        let resolved = resolve_cwd_under_workspace(dir.path(), Path::new("notebooks")).expect("allow");
        assert_eq!(resolved, nested.canonicalize().unwrap());
    }

    #[test]
    fn rejects_path_outside_workspace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let err = resolve_cwd_under_workspace(dir.path(), outside.path()).expect_err("deny");
        assert!(matches!(err, KernelError::CwdNotAllowed { .. }));
    }

    #[test]
    fn rejects_missing_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope");
        let err = resolve_cwd_under_workspace(dir.path(), &missing).expect_err("deny");
        assert!(matches!(err, KernelError::CwdNotAllowed { .. }));
    }
}
