use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

fn unique_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!(
        "{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn temporary_sibling(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().ok_or_else(|| Error::OutsideWorkspace {
        path: path.to_path_buf(),
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::OutsideWorkspace {
            path: path.to_path_buf(),
        })?;
    Ok(parent.join(format!(".{name}.lattice-tmp-{}", unique_suffix())))
}

/// Durably replace an arbitrary native file with a sibling temporary file.
///
/// The temporary file is unique per write, receives the existing target's
/// permissions, is flushed before replacement, and is cleaned up on failure.
/// Unix rename replacement is atomic. Windows uses `MoveFileExW` with explicit
/// replace-existing and write-through flags because `std::fs::rename` does not
/// provide portable replacement semantics there.
pub fn atomic_write_file(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;

    let parent = path.parent().ok_or_else(|| Error::OutsideWorkspace {
        path: path.to_path_buf(),
    })?;
    std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
    let temporary = temporary_sibling(path)?;
    let existing_permissions = std::fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());

    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        if let Some(permissions) = existing_permissions {
            file.set_permissions(permissions)?;
        }
        file.sync_all()?;
        replace_file(&temporary, path)?;
        if let Ok(directory) = std::fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(Error::io(path, error));
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let from: Vec<u16> = from
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let to: Vec<u16> = to
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn replaces_file_and_preserves_permissions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.yaml");
        std::fs::write(&path, "old").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        }

        atomic_write_file(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
    }

    #[test]
    fn concurrent_writers_do_not_collide_on_temporary_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = Arc::new(directory.path().join("settings.yaml"));
        let threads = (0..8)
            .map(|index| {
                let path = Arc::clone(&path);
                std::thread::spawn(move || {
                    atomic_write_file(&path, format!("writer-{index}").as_bytes())
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap().unwrap();
        }
        let value = std::fs::read_to_string(&*path).unwrap();
        assert!(value.starts_with("writer-"));
        assert!(std::fs::read_dir(directory.path())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("lattice-tmp")));
    }
}
