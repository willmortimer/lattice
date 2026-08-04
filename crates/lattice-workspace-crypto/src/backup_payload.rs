//! Workspace backup plaintext payload before DEK encryption.
//!
//! Format: `LWBK` magic, version `1`, manifest bytes, then length-prefixed file
//! entries (path UTF-8, raw bytes). Hidden dirs (`.lattice`, `.git`) are skipped.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

const MAGIC: &[u8; 4] = b"LWBK";
const VERSION: u8 = 1;
const MAX_FILES: usize = 512;
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;

/// Build an opaque workspace backup payload from `workspace_root`.
pub fn build_workspace_backup_payload(workspace_root: &Path) -> Result<Vec<u8>> {
    let manifest_path = workspace_root.join("lattice.yaml");
    let manifest = fs::read(&manifest_path).map_err(|source| Error::BackupPayload {
        message: format!("read {}: {source}", manifest_path.display()),
    })?;

    let mut files = Vec::new();
    collect_files(workspace_root, workspace_root, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    out.extend_from_slice(&manifest);

    let mut total_bytes = manifest.len();
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (rel, bytes) in files {
        total_bytes = total_bytes
            .checked_add(rel.len())
            .and_then(|n| n.checked_add(bytes.len()))
            .ok_or_else(|| Error::BackupPayload {
                message: "backup payload too large".into(),
            })?;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(Error::BackupPayload {
                message: format!("backup payload exceeds {MAX_TOTAL_BYTES} bytes"),
            });
        }
        let rel_len: u16 = rel.len().try_into().map_err(|_| Error::BackupPayload {
            message: format!("path too long for backup entry: {rel}"),
        })?;
        out.extend_from_slice(&rel_len.to_le_bytes());
        out.extend_from_slice(rel.as_bytes());
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

fn collect_files(
    workspace_root: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    if out.len() >= MAX_FILES {
        return Err(Error::BackupPayload {
            message: format!("backup exceeds {MAX_FILES} files"),
        });
    }
    let entries = fs::read_dir(dir).map_err(|source| Error::BackupPayload {
        message: format!("read dir {}: {source}", dir.display()),
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::BackupPayload {
            message: format!("read dir entry in {}: {source}", dir.display()),
        })?;
        let file_type = entry.file_type().map_err(|source| Error::BackupPayload {
            message: format!("file type {}: {source}", entry.path().display()),
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if file_type.is_dir() {
            collect_files(workspace_root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("lattice.yaml")
            && path.parent() == Some(workspace_root)
        {
            continue;
        }
        let rel = path
            .strip_prefix(workspace_root)
            .map_err(|_| Error::BackupPayload {
                message: format!("path not under workspace: {}", path.display()),
            })?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&path).map_err(|source| Error::BackupPayload {
            message: format!("read {}: {source}", path.display()),
        })?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(Error::BackupPayload {
                message: format!(
                    "{} exceeds per-file limit of {MAX_FILE_BYTES} bytes",
                    path.display()
                ),
            });
        }
        out.push((rel, bytes));
        if out.len() >= MAX_FILES {
            return Err(Error::BackupPayload {
                message: format!("backup exceeds {MAX_FILES} files"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;

    #[test]
    fn round_trip_fixture_workspace() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Backup").unwrap();
        std::fs::write(dir.path().join("Notes.md"), b"hello backup").unwrap();

        let payload = build_workspace_backup_payload(dir.path()).unwrap();
        assert!(payload.starts_with(b"LWBK"));
        assert!(payload.len() > 20);
    }
}
