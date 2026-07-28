//! Materialize a run directory tree from an [`ExecutionManifest`].

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::manifest::ExecutionManifest;

/// Materialized run directory with standard KernelFS layout.
#[derive(Debug, Clone)]
pub struct RunDir {
    pub root: PathBuf,
    pub hydration: HydrationRecord,
}

/// Provenance record emitted after materialization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydrationRecord {
    pub run_id: String,
    pub base_snapshot: String,
    pub root: PathBuf,
    pub sources: Vec<HydrationSource>,
}

/// One hydrated input file with content hash.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydrationSource {
    pub guest_path: String,
    pub host_path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MaterializeError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path escape rejected for guest path {guest_path:?}: {reason}")]
    PathEscape { guest_path: String, reason: String },
    #[error("input mount guest path must be relative: {guest_path:?}")]
    InvalidGuestPath { guest_path: String },
    #[error("input host path does not exist: {path}")]
    MissingHostPath { path: PathBuf },
}

/// Create `input/`, `work/`, `output/`, and `tmp/` under `parent` and hydrate inputs.
pub fn materialize(parent: &Path, manifest: &ExecutionManifest) -> Result<RunDir, MaterializeError> {
    let root = parent.join(&manifest.run_id);
    for subdir in ["input", "work", "output", "tmp"] {
        let path = root.join(subdir);
        fs::create_dir_all(&path).map_err(|source| MaterializeError::Io {
            path: path.clone(),
            source,
        })?;
    }

    let mut sources = Vec::new();
    for mount in &manifest.mounts.input {
        let guest_rel = normalize_guest_path(&mount.guest_path)?;
        let dest = root.join("input").join(&guest_rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|source| MaterializeError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        if !mount.host_path.exists() {
            return Err(MaterializeError::MissingHostPath {
                path: mount.host_path.clone(),
            });
        }

        fs::copy(&mount.host_path, &dest).map_err(|source| MaterializeError::Io {
            path: dest.clone(),
            source,
        })?;

        let sha256 = hash_file(&dest)?;
        sources.push(HydrationSource {
            guest_path: guest_rel.to_string_lossy().replace('\\', "/"),
            host_path: mount.host_path.clone(),
            sha256,
        });
    }

    let hydration = HydrationRecord {
        run_id: manifest.run_id.clone(),
        base_snapshot: manifest.base_snapshot.clone(),
        root: root.clone(),
        sources,
    };

    Ok(RunDir { root, hydration })
}

fn hash_file(path: &Path) -> Result<String, MaterializeError> {
    let mut file = fs::File::open(path).map_err(|source| MaterializeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|source| MaterializeError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Reject absolute paths and `..` components in guest-relative mount targets.
pub fn normalize_guest_path(guest_path: &str) -> Result<PathBuf, MaterializeError> {
    let text = guest_path.trim().replace('\\', "/");
    if text.is_empty() {
        return Err(MaterializeError::InvalidGuestPath {
            guest_path: guest_path.to_string(),
        });
    }

    let candidate = PathBuf::from(&text);
    if candidate.is_absolute() {
        return Err(MaterializeError::PathEscape {
            guest_path: guest_path.to_string(),
            reason: "guest path must be relative".into(),
        });
    }

    for component in candidate.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                return Err(MaterializeError::PathEscape {
                    guest_path: guest_path.to_string(),
                    reason: "guest path must not contain `..`".into(),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(MaterializeError::PathEscape {
                    guest_path: guest_path.to_string(),
                    reason: "guest path must not be absolute".into(),
                });
            }
        }
    }

    Ok(candidate)
}
