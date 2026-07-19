use std::fs;
use std::path::{Path, PathBuf};

use lattice_embedding::{
    file_sha256_hex, verify_file_sha256, EmbeddingInstallState, ModelManifest,
    MANIFEST_SCHEMA_VERSION,
};

use crate::error::EmbedHostError;

/// Result of an explicit model install into the models directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    pub model_dir: PathBuf,
    pub artifact_sha256: String,
    pub install_state: EmbeddingInstallState,
}

/// Install a model by copying a verified artifact beside its manifest.
///
/// Never downloads. Callers must supply local `manifest_path` and
/// `artifact_path`. The artifact is sha256-verified against the manifest before
/// being copied into `<models_dir>/<slug>/`.
pub fn install_model(
    manifest_path: &Path,
    artifact_path: &Path,
    models_dir: &Path,
) -> Result<InstallResult, EmbedHostError> {
    let manifest_bytes = fs::read(manifest_path).map_err(|error| {
        EmbedHostError::protocol(format!(
            "failed to read manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: ModelManifest = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        EmbedHostError::protocol(format!("invalid manifest JSON: {error}"))
    })?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(EmbedHostError::protocol(format!(
            "unsupported manifest schema version {}",
            manifest.schema_version
        )));
    }
    manifest.validate()?;

    verify_file_sha256(artifact_path, &manifest.sha256)?;

    let slug = model_slug(&manifest.model_id);
    let model_dir = models_dir.join(&slug);
    fs::create_dir_all(&model_dir).map_err(|error| {
        EmbedHostError::protocol(format!(
            "failed to create model dir {}: {error}",
            model_dir.display()
        ))
    })?;

    let dest_manifest = model_dir.join("manifest.json");
    let dest_artifact = model_dir.join(&manifest.artifact);
    fs::write(&dest_manifest, &manifest_bytes).map_err(|error| {
        EmbedHostError::protocol(format!(
            "failed to write manifest {}: {error}",
            dest_manifest.display()
        ))
    })?;
    fs::copy(artifact_path, &dest_artifact).map_err(|error| {
        EmbedHostError::protocol(format!(
            "failed to copy artifact to {}: {error}",
            dest_artifact.display()
        ))
    })?;

    // Re-verify the installed copy so a partial write cannot be treated as ready.
    verify_file_sha256(&dest_artifact, &manifest.sha256)?;
    let artifact_sha256 = file_sha256_hex(&dest_artifact)?;

    Ok(InstallResult {
        model_dir,
        artifact_sha256,
        install_state: EmbeddingInstallState::NotInstalled,
    })
}

/// Load and validate a manifest from an installed model directory.
pub fn load_manifest(model_dir: &Path) -> Result<(ModelManifest, PathBuf), EmbedHostError> {
    let manifest_path = model_dir.join("manifest.json");
    let bytes = fs::read(&manifest_path).map_err(|_| {
        lattice_embedding::EmbeddingError::ArtifactNotFound {
            path: manifest_path.display().to_string(),
        }
    })?;
    let manifest: ModelManifest = serde_json::from_slice(&bytes)
        .map_err(|error| EmbedHostError::protocol(format!("invalid manifest JSON: {error}")))?;
    manifest.validate()?;
    let artifact_path = model_dir.join(&manifest.artifact);
    if !artifact_path.is_file() {
        return Err(lattice_embedding::EmbeddingError::ArtifactNotFound {
            path: artifact_path.display().to_string(),
        }
        .into());
    }
    verify_file_sha256(&artifact_path, &manifest.sha256)?;
    Ok((manifest, artifact_path))
}

fn model_slug(model_id: &str) -> String {
    let trimmed = model_id.trim();
    let leaf = trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    let slug: String = leaf
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if slug.is_empty() {
        "model".into()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_embedding::{sha256_hex, PoolingStrategy};
    use std::io::Write;
    use tempfile::tempdir;

    fn sample_manifest(sha: &str) -> ModelManifest {
        ModelManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            provider: "fake".into(),
            model_id: "Qwen/Qwen3-Embedding-0.6B-GGUF".into(),
            model_revision: "rev-test".into(),
            artifact: "fixture.bin".into(),
            sha256: sha.to_string(),
            license: "Apache-2.0".into(),
            native_dimensions: 1024,
            default_dimensions: 512,
            pooling: PoolingStrategy::Last,
            instruction_version: "lattice-retrieval-v1".into(),
        }
    }

    #[test]
    fn install_verifies_sha_and_copies() {
        let dir = tempdir().unwrap();
        let artifact = dir.path().join("source.bin");
        let mut file = fs::File::create(&artifact).unwrap();
        write!(file, "fixture-bytes").unwrap();
        drop(file);
        let sha = sha256_hex(b"fixture-bytes");
        let manifest = sample_manifest(&sha);
        let manifest_path = dir.path().join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let models = dir.path().join("models");
        let result = install_model(&manifest_path, &artifact, &models).unwrap();
        assert!(result.model_dir.join("manifest.json").is_file());
        assert!(result.model_dir.join("fixture.bin").is_file());
        assert_eq!(result.artifact_sha256, sha);

        let (loaded, path) = load_manifest(&result.model_dir).unwrap();
        assert_eq!(loaded.model_id, manifest.model_id);
        assert_eq!(path, result.model_dir.join("fixture.bin"));
    }

    #[test]
    fn install_rejects_sha_mismatch() {
        let dir = tempdir().unwrap();
        let artifact = dir.path().join("source.bin");
        fs::write(&artifact, b"fixture-bytes").unwrap();
        let manifest = sample_manifest("deadbeef");
        let manifest_path = dir.path().join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let err = install_model(&manifest_path, &artifact, &dir.path().join("models")).unwrap_err();
        assert!(matches!(
            err,
            EmbedHostError::Embedding(
                lattice_embedding::EmbeddingError::ArtifactSha256Mismatch { .. }
            )
        ));
    }
}
