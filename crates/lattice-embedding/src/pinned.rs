//! Pinned Qwen3 embedding GGUF used for semantic search enablement (E5).
//!
//! Download happens only on explicit enable — never inside a search request.
//! CI stays offline via [`crate::ENV_SEMANTIC_FAKE`] or
//! [`crate::ENV_SEMANTIC_MODEL_SOURCE`].

use crate::manifest::{ModelManifest, MANIFEST_SCHEMA_VERSION};
use crate::specification::PoolingStrategy;

/// HuggingFace model id for the pinned GGUF package.
pub const QWEN3_EMBEDDING_MODEL_ID: &str = "Qwen/Qwen3-Embedding-0.6B-GGUF";

/// Pinned HuggingFace git revision for the GGUF artifact.
pub const QWEN3_EMBEDDING_MODEL_REVISION: &str = "370f27d7550e0def9b39c1f16d3fbaa13aa67728";

/// On-disk artifact filename.
pub const QWEN3_EMBEDDING_ARTIFACT: &str = "Qwen3-Embedding-0.6B-Q8_0.gguf";

/// Lowercase hex sha256 for [`QWEN3_EMBEDDING_ARTIFACT`] at the pinned revision.
pub const QWEN3_EMBEDDING_SHA256: &str =
    "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439";

/// Exact byte size of the pinned Q8 GGUF (~640 MB).
pub const QWEN3_EMBEDDING_SIZE_BYTES: u64 = 639_150_592;

/// Human-facing approximate size for confirm dialogs.
pub const QWEN3_EMBEDDING_SIZE_LABEL: &str = "~640 MB";

/// Apache-2.0 license id shown before download.
pub const QWEN3_EMBEDDING_LICENSE: &str = "Apache-2.0";

/// Install directory leaf under `Models/embeddings/`.
pub const QWEN3_EMBEDDING_INSTALL_SLUG: &str = "qwen3-embedding-0.6b";

/// HTTPS resolve URL for the pinned revision (follows CDN redirects).
pub const QWEN3_EMBEDDING_DOWNLOAD_URL: &str = concat!(
    "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/",
    "370f27d7550e0def9b39c1f16d3fbaa13aa67728/",
    "Qwen3-Embedding-0.6B-Q8_0.gguf"
);

/// Build the pinned on-disk manifest for install / load.
pub fn qwen3_embedding_0_6b_q8_manifest() -> ModelManifest {
    ModelManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        provider: "llama.cpp".into(),
        model_id: QWEN3_EMBEDDING_MODEL_ID.into(),
        model_revision: QWEN3_EMBEDDING_MODEL_REVISION.into(),
        artifact: QWEN3_EMBEDDING_ARTIFACT.into(),
        sha256: QWEN3_EMBEDDING_SHA256.into(),
        license: QWEN3_EMBEDDING_LICENSE.into(),
        native_dimensions: 1024,
        default_dimensions: 512,
        pooling: PoolingStrategy::Last,
        instruction_version: "lattice-retrieval-v1".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_manifest_validates() {
        qwen3_embedding_0_6b_q8_manifest().validate().unwrap();
    }

    #[test]
    fn pinned_url_targets_revision_and_artifact() {
        assert!(QWEN3_EMBEDDING_DOWNLOAD_URL.contains(QWEN3_EMBEDDING_MODEL_REVISION));
        assert!(QWEN3_EMBEDDING_DOWNLOAD_URL.contains(QWEN3_EMBEDDING_ARTIFACT));
        assert_eq!(QWEN3_EMBEDDING_SHA256.len(), 64);
        assert_eq!(QWEN3_EMBEDDING_SIZE_BYTES, 639_150_592);
    }
}
