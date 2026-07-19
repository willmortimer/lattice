//! Optional llama.cpp + Metal backend.
//!
//! This module is compiled only with `--features llama-cpp`. The first landing
//! keeps a clear "not yet linked" surface so CI never downloads the ~639MB
//! Qwen3 GGUF. See `apps/embed-host/README.md` for enabling a real build.

use std::path::Path;

use async_trait::async_trait;
use lattice_embedding::{
    DistanceMetric, EmbedDocumentRequest, EmbedQueryRequest, EmbeddingError, EmbeddingProvider,
    EmbeddingSpecification, EmbeddingVector, ModelManifest,
};

use super::{BackendKind, EmbeddingBackend};
use crate::error::EmbedHostError;

/// Placeholder llama.cpp provider. Returns an explicit unavailable error until
/// llama.cpp is linked into this crate.
pub struct LlamaCppBackend {
    specification: EmbeddingSpecification,
}

impl LlamaCppBackend {
    pub fn open(
        manifest: &ModelManifest,
        artifact_path: &Path,
        dimensions: u32,
    ) -> Result<Self, EmbedHostError> {
        // Artifact presence was already verified by install/load_manifest.
        if !artifact_path.is_file() {
            return Err(lattice_embedding::EmbeddingError::ArtifactNotFound {
                path: artifact_path.display().to_string(),
            }
            .into());
        }
        let mut specification =
            manifest.to_specification(dimensions, DistanceMetric::Cosine, true)?;
        specification.provider_id = "llama.cpp".into();
        Err(EmbedHostError::BackendUnavailable(format!(
            "llama-cpp feature is enabled but no llama.cpp bindings are linked yet \
             (model {} at {}). See apps/embed-host/README.md to wire Metal GGUF inference.",
            specification.model_id,
            artifact_path.display()
        )))
    }
}

#[async_trait]
impl EmbeddingProvider for LlamaCppBackend {
    fn specification(&self) -> &EmbeddingSpecification {
        &self.specification
    }

    async fn embed_query(
        &self,
        _request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError> {
        Err(EmbeddingError::provider(
            "llama.cpp backend is not linked; see apps/embed-host/README.md",
        ))
    }

    async fn embed_documents(
        &self,
        _requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        Err(EmbeddingError::provider(
            "llama.cpp backend is not linked; see apps/embed-host/README.md",
        ))
    }
}

#[async_trait]
impl EmbeddingBackend for LlamaCppBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::LlamaCpp
    }
}
