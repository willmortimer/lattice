//! Model cache paths for embedding artifacts.

use std::path::PathBuf;

use crate::pinned::QWEN3_EMBEDDING_INSTALL_SLUG;

/// Env override for an isolated Lattice profile root (`LATTICE_DEV_HOME` /
/// `LATTICE_HOME`). When set, models live under `{home}/Models`.
pub const ENV_LATTICE_DEV_HOME: &str = "LATTICE_DEV_HOME";
pub const ENV_LATTICE_HOME: &str = "LATTICE_HOME";

/// Root directory for Lattice model artifacts (`…/Lattice/Models`).
///
/// Prefer a profile home when `LATTICE_DEV_HOME` or `LATTICE_HOME` is set
/// (dev). Otherwise use the platform data dir
/// (`~/Library/Application Support/Lattice/Models` on macOS).
pub fn models_root() -> PathBuf {
    if let Some(home) = profile_home_override() {
        return home.join("Models");
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join("Models")
}

/// Directory for embedding provider installs: `{models_root}/embeddings`.
pub fn embeddings_models_dir() -> PathBuf {
    models_root().join("embeddings")
}

/// Install directory for the pinned Qwen3 embedding model.
pub fn qwen3_embedding_install_dir() -> PathBuf {
    embeddings_models_dir().join(QWEN3_EMBEDDING_INSTALL_SLUG)
}

/// Staging directory for in-progress downloads.
pub fn embeddings_download_dir() -> PathBuf {
    models_root().join("downloads").join("embeddings")
}

fn profile_home_override() -> Option<PathBuf> {
    for key in [ENV_LATTICE_DEV_HOME, ENV_LATTICE_HOME] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeddings_dir_nests_under_models() {
        let root = models_root();
        assert!(embeddings_models_dir().starts_with(&root));
        assert!(qwen3_embedding_install_dir().ends_with(QWEN3_EMBEDDING_INSTALL_SLUG));
    }
}
