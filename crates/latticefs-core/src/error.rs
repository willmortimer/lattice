use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("path escapes workspace: {path}")]
    OutsideWorkspace { path: PathBuf },

    #[error("resource not found at path: {path}")]
    ResourceNotFound { path: String },

    #[error("resource id not found: {resource_id}")]
    ResourceIdNotFound { resource_id: String },

    #[error("path already registered: {path}")]
    PathAlreadyRegistered { path: String },

    #[error("rename source not found: {from}")]
    RenameSourceNotFound { from: String },

    #[error("rename destination already exists: {to}")]
    RenameDestinationExists { to: String },

    #[error("invalid content hash: {value}")]
    InvalidContentHash { value: String },

    #[error("cloud blob error: {message}")]
    CloudBlob { message: String },

    #[error("blob not found for resource: {resource_id}")]
    BlobNotFound { resource_id: String },

    #[error("blob already exists for resource: {resource_id}")]
    BlobAlreadyExists { resource_id: String },

    #[error("blob hash mismatch: expected {expected}, got {actual}")]
    BlobHashMismatch {
        expected: crate::types::ContentHash,
        actual: crate::types::ContentHash,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Normalize a workspace-relative path to a POSIX-style string key.
pub(crate) fn normalize_path_key(path: &str) -> Result<String> {
    let parsed = Path::new(path);
    let mut out = PathBuf::new();
    for component in parsed.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::OutsideWorkspace {
                    path: parsed.to_path_buf(),
                });
            }
        }
    }
    let key = out
        .to_str()
        .ok_or_else(|| Error::OutsideWorkspace {
            path: parsed.to_path_buf(),
        })?
        .replace('\\', "/");
    if key.is_empty() {
        return Err(Error::OutsideWorkspace {
            path: parsed.to_path_buf(),
        });
    }
    Ok(key)
}
