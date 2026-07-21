use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Find an executable named `name` on a `PATH`-style search path.
///
/// On Windows, also tries `name.exe` when `name` has no extension.
pub(crate) fn find_on_path(name: &str, path_env: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(path_env) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            if Path::new(name).extension().is_none() {
                let with_exe = dir.join(format!("{name}.exe"));
                if is_executable(&with_exe) {
                    return Some(with_exe);
                }
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(meta) => meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn finds_first_match_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("python3");
        fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms).unwrap();

        let empty = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([empty.path(), dir.path()]).unwrap();

        let found = find_on_path("python3", &path).unwrap();
        assert_eq!(found, bin);
    }

    #[test]
    fn missing_when_not_on_path() {
        let empty = tempfile::tempdir().unwrap();
        let path = empty.path().as_os_str();
        assert!(find_on_path("python3", path).is_none());
    }
}
