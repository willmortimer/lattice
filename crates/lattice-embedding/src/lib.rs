//! Provider-neutral embedding contract and model manifests for Lattice.
//!
//! This crate defines the shared provider trait, specification types, model
//! manifests, and deterministic test providers. Runtime backends such as
//! llama.cpp and Core ML implement the trait outside this crate.

mod error;
mod manifest;
mod provider;
mod specification;
mod status;

pub use error::EmbeddingError;
pub use manifest::{
    file_sha256_hex, sha256_hex, verify_file_sha256, ModelManifest, MANIFEST_SCHEMA_VERSION,
};
pub use provider::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider, EmbeddingVector,
    FakeEmbeddingProvider,
};
pub use specification::{DistanceMetric, EmbeddingSpecification, PoolingStrategy};
pub use status::{ChunkEmbeddingStatus, EmbeddingInstallState};
