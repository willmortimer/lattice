use async_trait::async_trait;
use lattice_embedding::{
    DistanceMetric, EmbedDocumentRequest, EmbedQueryRequest, EmbeddingError, EmbeddingProvider,
    EmbeddingSpecification, EmbeddingVector, FakeEmbeddingProvider, ModelManifest,
};

use super::{BackendKind, EmbeddingBackend};
use crate::error::EmbedHostError;

/// Deterministic in-process backend for CI and protocol tests.
pub struct FakeBackend {
    provider: FakeEmbeddingProvider,
}

impl FakeBackend {
    pub fn from_manifest(
        manifest: &ModelManifest,
        dimensions: u32,
    ) -> Result<Self, EmbedHostError> {
        let specification =
            manifest.to_specification(dimensions, DistanceMetric::Cosine, true)?;
        // Keep provider_id reflecting the active host backend for namespace keys.
        let mut specification = specification;
        specification.provider_id = "fake".into();
        Ok(Self {
            provider: FakeEmbeddingProvider::new(specification),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for FakeBackend {
    fn specification(&self) -> &EmbeddingSpecification {
        self.provider.specification()
    }

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError> {
        self.provider.embed_query(request).await
    }

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        self.provider.embed_documents(requests).await
    }
}

#[async_trait]
impl EmbeddingBackend for FakeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Fake
    }
}
