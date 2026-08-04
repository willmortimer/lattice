//! Append-only Yrs update journal + snapshot under `.lattice/collab/<uuid>/`.
//!
//! On-disk layout (per resource):
//! - `snapshot.bin` — full document state as a lib0 v1 update from the empty SV
//! - `updates.bin` — length-prefixed (`u32` BE + bytes) append-only update frames
//!
//! Reopen = apply snapshot (if present) then replay `updates.bin` frames in order.
//! Compaction folds the live state into a new snapshot and truncates the log.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use latticefs_core::{ResourceId, OPERATIONAL_DIR};

use crate::error::{Error, Result};

/// Subdirectory of `.lattice` holding per-resource collab journals.
pub const COLLAB_SUBDIR: &str = "collab";
pub const SNAPSHOT_FILENAME: &str = "snapshot.bin";
pub const UPDATES_FILENAME: &str = "updates.bin";

/// Directory for one resource's journal: `{root}/.lattice/collab/{uuid}/`.
pub fn journal_dir(workspace_root: &Path, resource_id: ResourceId) -> PathBuf {
    workspace_root
        .join(OPERATIONAL_DIR)
        .join(COLLAB_SUBDIR)
        .join(resource_id.to_string())
}

/// True when a snapshot file exists or the updates log has any bytes.
pub fn journal_exists(dir: &Path) -> bool {
    let snapshot = dir.join(SNAPSHOT_FILENAME);
    if snapshot.is_file() {
        return true;
    }
    let updates = dir.join(UPDATES_FILENAME);
    updates
        .metadata()
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Ensure the journal directory exists.
pub fn ensure_journal_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|err| Error::Io {
        path: dir.display().to_string(),
        message: err.to_string(),
    })
}

/// Read `snapshot.bin` if present (empty file → empty vec).
pub fn read_snapshot(dir: &Path) -> Result<Option<Vec<u8>>> {
    let path = dir.join(SNAPSHOT_FILENAME);
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(Error::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        }),
    }
}

/// Atomically replace `snapshot.bin` with `bytes` (temp + rename + fsync).
pub fn write_snapshot(dir: &Path, bytes: &[u8]) -> Result<()> {
    ensure_journal_dir(dir)?;
    let path = dir.join(SNAPSHOT_FILENAME);
    atomic_write(&path, bytes)
}

/// Append one length-prefixed update frame to `updates.bin`, then fsync.
pub fn append_update(dir: &Path, update: &[u8]) -> Result<()> {
    ensure_journal_dir(dir)?;
    let path = dir.join(UPDATES_FILENAME);
    let len = u32::try_from(update.len()).map_err(|_| Error::Io {
        path: path.display().to_string(),
        message: format!("update too large: {} bytes", update.len()),
    })?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| Error::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
    file.write_all(&len.to_be_bytes()).map_err(|err| Error::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    file.write_all(update).map_err(|err| Error::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    file.sync_all().map_err(|err| Error::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(())
}

/// Decode all frames from `updates.bin` (missing file → empty).
pub fn read_updates(dir: &Path) -> Result<Vec<Vec<u8>>> {
    let path = dir.join(UPDATES_FILENAME);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(Error::Io {
                path: path.display().to_string(),
                message: err.to_string(),
            })
        }
    };
    decode_update_frames(&bytes, &path)
}

/// Truncate the updates log to empty (after a successful snapshot write).
pub fn truncate_updates(dir: &Path) -> Result<()> {
    ensure_journal_dir(dir)?;
    let path = dir.join(UPDATES_FILENAME);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|err| Error::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
    file.sync_all().map_err(|err| Error::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(())
}

/// Write a new snapshot and truncate the append log.
pub fn compact_to_snapshot(dir: &Path, full_state_update: &[u8]) -> Result<()> {
    write_snapshot(dir, full_state_update)?;
    truncate_updates(dir)?;
    Ok(())
}

fn decode_update_frames(bytes: &[u8], path: &Path) -> Result<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if bytes.len() - offset < 4 {
            return Err(Error::Io {
                path: path.display().to_string(),
                message: format!("truncated updates.bin at offset {offset}"),
            });
        }
        let len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;
        if bytes.len() - offset < len {
            return Err(Error::Io {
                path: path.display().to_string(),
                message: format!("truncated update frame at offset {offset} (need {len} bytes)"),
            });
        }
        out.push(bytes[offset..offset + len].to_vec());
        offset += len;
    }
    Ok(out)
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| Error::Io {
        path: path.display().to_string(),
        message: "snapshot path has no parent".into(),
    })?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("snapshot.bin"),
        std::process::id()
    ));
    let write_result = (|| -> std::io::Result<()> {
        {
            let mut file = File::create(&tmp)?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(Error::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn append_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let jdir = dir.path().join("j");
        append_update(&jdir, b"aaa").unwrap();
        append_update(&jdir, b"bbbb").unwrap();
        let frames = read_updates(&jdir).unwrap();
        assert_eq!(frames, vec![b"aaa".to_vec(), b"bbbb".to_vec()]);
    }

    #[test]
    fn compact_clears_updates() {
        let dir = tempdir().unwrap();
        let jdir = dir.path().join("j");
        append_update(&jdir, b"u1").unwrap();
        compact_to_snapshot(&jdir, b"snap").unwrap();
        assert_eq!(read_snapshot(&jdir).unwrap().as_deref(), Some(b"snap".as_slice()));
        assert!(read_updates(&jdir).unwrap().is_empty());
        assert!(journal_exists(&jdir));
    }
}
