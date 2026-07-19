use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::EmbeddingError;
use crate::specification::{DistanceMetric, EmbeddingSpecification, PoolingStrategy};

/// Current on-disk manifest schema version.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Pinned model artifact manifest stored beside downloaded weights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelManifest {
    pub schema_version: u32,
    pub provider: String,
    pub model_id: String,
    pub model_revision: String,
    pub artifact: String,
    pub sha256: String,
    pub license: String,
    pub native_dimensions: u32,
    pub default_dimensions: u32,
    pub pooling: PoolingStrategy,
    pub instruction_version: String,
}

impl ModelManifest {
    pub fn validate(&self) -> Result<(), EmbeddingError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(EmbeddingError::manifest(format!(
                "unsupported schema version {}",
                self.schema_version
            )));
        }
        if self.model_id.trim().is_empty() {
            return Err(EmbeddingError::manifest("model_id must not be empty"));
        }
        if self.artifact.trim().is_empty() {
            return Err(EmbeddingError::manifest("artifact must not be empty"));
        }
        if self.sha256.trim().is_empty() {
            return Err(EmbeddingError::manifest("sha256 must not be empty"));
        }
        if self.default_dimensions == 0 || self.native_dimensions == 0 {
            return Err(EmbeddingError::manifest("dimensions must be non-zero"));
        }
        if self.default_dimensions > self.native_dimensions {
            return Err(EmbeddingError::manifest(
                "default_dimensions cannot exceed native_dimensions",
            ));
        }
        Ok(())
    }

    pub fn to_specification(
        &self,
        dimensions: u32,
        distance: DistanceMetric,
        normalized: bool,
    ) -> Result<EmbeddingSpecification, EmbeddingError> {
        self.validate()?;
        if dimensions == 0 || dimensions > self.native_dimensions {
            return Err(EmbeddingError::InvalidDimensions {
                requested: dimensions,
                supported: self.native_dimensions,
            });
        }
        Ok(EmbeddingSpecification {
            provider_id: self.provider.clone(),
            model_id: self.model_id.clone(),
            model_revision: self.model_revision.clone(),
            artifact_sha256: self.sha256.clone(),
            dimensions,
            native_dimensions: self.native_dimensions,
            distance,
            pooling: self.pooling,
            normalized,
            instruction_version: self.instruction_version.clone(),
        })
    }
}

/// Return lowercase hex sha256 for `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

/// Hash a file on disk and compare it to an expected lowercase hex digest.
pub fn verify_file_sha256(path: &Path, expected_sha256: &str) -> Result<(), EmbeddingError> {
    let actual = file_sha256_hex(path)?;
    let expected = expected_sha256.trim().to_ascii_lowercase();
    if actual != expected {
        return Err(EmbeddingError::ArtifactSha256Mismatch {
            expected,
            actual,
        });
    }
    Ok(())
}

/// Return lowercase hex sha256 for a file on disk.
pub fn file_sha256_hex(path: &Path) -> Result<String, EmbeddingError> {
    let file = File::open(path).map_err(|_| EmbeddingError::ArtifactNotFound {
        path: path.display().to_string(),
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| EmbeddingError::provider(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn sample_manifest() -> ModelManifest {
        ModelManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            provider: "llama.cpp".into(),
            model_id: "Qwen/Qwen3-Embedding-0.6B-GGUF".into(),
            model_revision: "rev-1".into(),
            artifact: "Qwen3-Embedding-0.6B-Q8_0.gguf".into(),
            sha256: "abc123".into(),
            license: "Apache-2.0".into(),
            native_dimensions: 1024,
            default_dimensions: 512,
            pooling: PoolingStrategy::Last,
            instruction_version: "lattice-retrieval-v1".into(),
        }
    }

    #[test]
    fn manifest_round_trips_json() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: ModelManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn verify_file_sha256_detects_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "hello").unwrap();
        let actual = file_sha256_hex(file.path()).unwrap();
        let err = verify_file_sha256(file.path(), "deadbeef").unwrap_err();
        assert!(matches!(
            err,
            EmbeddingError::ArtifactSha256Mismatch { .. }
        ));
        verify_file_sha256(file.path(), &actual).unwrap();
    }
}
