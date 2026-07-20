use serde::{Deserialize, Serialize};

/// Distance metric used when comparing stored vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    Cosine,
    Dot,
    L2,
}

/// Pooling strategy used to collapse token embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolingStrategy {
    Last,
    Mean,
    Cls,
}

/// Provider-neutral description of one embedding namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpecification {
    pub provider_id: String,
    pub model_id: String,
    pub model_revision: String,
    pub artifact_sha256: String,
    pub dimensions: u32,
    pub native_dimensions: u32,
    pub distance: DistanceMetric,
    pub pooling: PoolingStrategy,
    pub normalized: bool,
    pub instruction_version: String,
}

impl EmbeddingSpecification {
    /// Stable namespace identity for index storage.
    ///
    /// The key includes `normalized`. Changing normalization (or any other
    /// field hashed here) produces a new namespace key and requires re-embed.
    pub fn namespace_key(&self, chunker_version: &str) -> String {
        let material = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.provider_id,
            self.model_id,
            self.model_revision,
            self.artifact_sha256,
            self.dimensions,
            self.native_dimensions,
            serde_json::to_string(&self.distance).unwrap_or_default(),
            serde_json::to_string(&self.pooling).unwrap_or_default(),
            self.normalized,
            self.instruction_version,
            chunker_version,
        );
        format!("sha256:{}", crate::manifest::sha256_hex(material.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec(normalized: bool) -> EmbeddingSpecification {
        EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:artifact".into(),
            dimensions: 8,
            native_dimensions: 8,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Last,
            normalized,
            instruction_version: "test-v1".into(),
        }
    }

    #[test]
    fn namespace_key_includes_normalized() {
        let normalized = sample_spec(true).namespace_key("lattice-chunker-v2");
        let unnormalized = sample_spec(false).namespace_key("lattice-chunker-v2");
        assert_ne!(normalized, unnormalized);
        assert_eq!(
            sample_spec(true).namespace_key("lattice-chunker-v2"),
            normalized
        );
    }
}
