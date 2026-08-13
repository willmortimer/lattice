//! Workspace backup plaintext payload before DEK encryption.
//!
//! Format: `LWBK` magic, version `1`, manifest bytes, then length-prefixed file
//! entries (path UTF-8, raw bytes). Hidden dirs (`.lattice`, `.git`) are skipped.

use std::fs;
use std::path::{Component, Path};

use crate::error::{Error, Result};

const MAGIC: &[u8; 4] = b"LWBK";
const VERSION: u8 = 1;
const MAX_FILES: usize = 512;
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;

/// Parsed `LWBK` backup payload (manifest + relative file entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPayload {
    pub manifest: Vec<u8>,
    pub files: Vec<(String, Vec<u8>)>,
}

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

/// Parse a `LWBK` backup payload built by [`build_workspace_backup_payload`].
///
/// Rejects path escape (`..`, absolute / prefixed paths) in file entries.
pub fn parse_workspace_backup_payload(bytes: &[u8]) -> Result<BackupPayload> {
    let mut cursor = 0usize;
    let magic = read_exact(bytes, &mut cursor, 4)?;
    if magic != MAGIC {
        return Err(Error::BackupPayload {
            message: "invalid backup magic (expected LWBK)".into(),
        });
    }
    let version = *read_exact(bytes, &mut cursor, 1)?
        .first()
        .ok_or_else(|| Error::BackupPayload {
            message: "truncated backup version".into(),
        })?;
    if version != VERSION {
        return Err(Error::BackupPayload {
            message: format!("unsupported backup version {version}"),
        });
    }

    let manifest_len = read_u32_le(bytes, &mut cursor)? as usize;
    if manifest_len > MAX_TOTAL_BYTES {
        return Err(Error::BackupPayload {
            message: format!("manifest exceeds {MAX_TOTAL_BYTES} bytes"),
        });
    }
    let manifest = read_exact(bytes, &mut cursor, manifest_len)?.to_vec();

    let file_count = read_u32_le(bytes, &mut cursor)? as usize;
    if file_count > MAX_FILES {
        return Err(Error::BackupPayload {
            message: format!("backup exceeds {MAX_FILES} files"),
        });
    }

    let mut total_bytes = manifest.len();
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let rel_len = read_u16_le(bytes, &mut cursor)? as usize;
        let rel_bytes = read_exact(bytes, &mut cursor, rel_len)?;
        let rel = std::str::from_utf8(rel_bytes).map_err(|source| Error::BackupPayload {
            message: format!("backup path is not UTF-8: {source}"),
        })?;
        validate_backup_rel_path(rel)?;

        let file_len = read_u32_le(bytes, &mut cursor)? as usize;
        if file_len > MAX_FILE_BYTES {
            return Err(Error::BackupPayload {
                message: format!("{rel} exceeds per-file limit of {MAX_FILE_BYTES} bytes"),
            });
        }
        total_bytes = total_bytes
            .checked_add(rel.len())
            .and_then(|n| n.checked_add(file_len))
            .ok_or_else(|| Error::BackupPayload {
                message: "backup payload too large".into(),
            })?;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(Error::BackupPayload {
                message: format!("backup payload exceeds {MAX_TOTAL_BYTES} bytes"),
            });
        }
        let file_bytes = read_exact(bytes, &mut cursor, file_len)?.to_vec();
        files.push((rel.to_string(), file_bytes));
    }

    if cursor != bytes.len() {
        return Err(Error::BackupPayload {
            message: format!(
                "trailing bytes after backup payload ({})",
                bytes.len() - cursor
            ),
        });
    }

    Ok(BackupPayload { manifest, files })
}

fn validate_backup_rel_path(rel: &str) -> Result<()> {
    if rel.trim().is_empty() {
        return Err(Error::BackupPayload {
            message: "backup path must be non-empty".into(),
        });
    }
    if rel.contains('\0') {
        return Err(Error::BackupPayload {
            message: format!("backup path contains NUL: {rel:?}"),
        });
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(Error::BackupPayload {
            message: format!("backup path must be relative: {rel:?}"),
        });
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(Error::BackupPayload {
            message: format!("backup path escapes restore root: {rel:?}"),
        });
    }
    Ok(())
}

fn read_exact<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| Error::BackupPayload {
            message: "backup payload length overflow".into(),
        })?;
    if end > bytes.len() {
        return Err(Error::BackupPayload {
            message: "truncated backup payload".into(),
        });
    }
    let slice = &bytes[*cursor..end];
    *cursor = end;
    Ok(slice)
}

fn read_u32_le(bytes: &[u8], cursor: &mut usize) -> Result<u32> {
    let raw = read_exact(bytes, cursor, 4)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn read_u16_le(bytes: &[u8], cursor: &mut usize) -> Result<u16> {
    let raw = read_exact(bytes, cursor, 2)?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
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
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("nested/a.txt"), b"nested").unwrap();

        let payload = build_workspace_backup_payload(dir.path()).unwrap();
        assert!(payload.starts_with(b"LWBK"));
        assert!(payload.len() > 20);

        let parsed = parse_workspace_backup_payload(&payload).unwrap();
        let expected_manifest = std::fs::read(dir.path().join("lattice.yaml")).unwrap();
        assert_eq!(parsed.manifest, expected_manifest);
        assert!(parsed
            .files
            .iter()
            .any(|(path, bytes)| path == "Notes.md" && bytes == b"hello backup"));
        assert!(parsed
            .files
            .iter()
            .any(|(path, bytes)| path == "nested/a.txt" && bytes == b"nested"));
    }

    #[test]
    fn parse_rejects_path_escape() {
        let mut crafted = Vec::new();
        crafted.extend_from_slice(MAGIC);
        crafted.push(VERSION);
        crafted.extend_from_slice(&1u32.to_le_bytes());
        crafted.push(b'x'); // manifest
        crafted.extend_from_slice(&1u32.to_le_bytes()); // one file
        let bad = b"../escape.txt";
        crafted.extend_from_slice(&(bad.len() as u16).to_le_bytes());
        crafted.extend_from_slice(bad);
        crafted.extend_from_slice(&4u32.to_le_bytes());
        crafted.extend_from_slice(b"data");

        let err = parse_workspace_backup_payload(&crafted).unwrap_err();
        assert!(err.to_string().contains("escapes"));
    }

    #[test]
    fn parse_rejects_absolute_path() {
        let mut crafted = Vec::new();
        crafted.extend_from_slice(MAGIC);
        crafted.push(VERSION);
        crafted.extend_from_slice(&1u32.to_le_bytes());
        crafted.push(b'x');
        crafted.extend_from_slice(&1u32.to_le_bytes());
        let bad = b"/etc/passwd";
        crafted.extend_from_slice(&(bad.len() as u16).to_le_bytes());
        crafted.extend_from_slice(bad);
        crafted.extend_from_slice(&4u32.to_le_bytes());
        crafted.extend_from_slice(b"data");

        let err = parse_workspace_backup_payload(&crafted).unwrap_err();
        assert!(err.to_string().contains("relative"));
    }
}
