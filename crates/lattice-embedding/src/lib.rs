//! Provider-neutral embedding contract and model manifests for Lattice.
//!
//! This crate defines the shared provider trait, specification types, model
//! manifests, and deterministic test providers. Runtime backends such as
//! llama.cpp and Core ML implement the trait outside this crate.
//!
//! Model download/install lives here so enablement hosts can stage the pinned
//! Qwen3 GGUF without pulling network work into search requests.

mod download;
mod error;
mod manifest;
mod paths;
mod pinned;
mod provider;
mod specification;
mod status;

pub use download::{
    acquire_pinned_embedding_model, download_progress_percent, install_artifact_beside_manifest,
    pinned_model_is_ready, semantic_fake_enabled, semantic_model_source_override, AcquireResult,
    ProgressFn, ENV_SEMANTIC_FAKE, ENV_SEMANTIC_MODEL_SOURCE,
};
pub use error::EmbeddingError;
pub use manifest::{
    file_sha256_hex, sha256_hex, verify_file_sha256, ModelManifest, MANIFEST_SCHEMA_VERSION,
};
pub use paths::{
    embeddings_download_dir, embeddings_models_dir, models_root, qwen3_embedding_install_dir,
    ENV_LATTICE_DEV_HOME, ENV_LATTICE_HOME,
};
pub use pinned::{
    qwen3_embedding_0_6b_q8_manifest, QWEN3_EMBEDDING_ARTIFACT, QWEN3_EMBEDDING_DOWNLOAD_URL,
    QWEN3_EMBEDDING_INSTALL_SLUG, QWEN3_EMBEDDING_LICENSE, QWEN3_EMBEDDING_MODEL_ID,
    QWEN3_EMBEDDING_MODEL_REVISION, QWEN3_EMBEDDING_SHA256, QWEN3_EMBEDDING_SIZE_BYTES,
    QWEN3_EMBEDDING_SIZE_LABEL,
};
pub use provider::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider, EmbeddingVector,
    FakeEmbeddingProvider,
};
pub use specification::{DistanceMetric, EmbeddingSpecification, PoolingStrategy};
pub use status::{ChunkEmbeddingStatus, EmbeddingInstallState};
