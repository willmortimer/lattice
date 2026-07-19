use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::error::EmbeddingError;
use crate::specification::EmbeddingSpecification;

/// One normalized embedding vector.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingVector {
    pub values: Vec<f32>,
}

/// Query embedding request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedQueryRequest {
    pub text: String,
}

/// Document embedding request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedDocumentRequest {
    pub chunk_id: String,
    pub text: String,
}

/// Provider-neutral embedding surface shared by llama.cpp, Core ML, and tests.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn specification(&self) -> &EmbeddingSpecification;

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError>;

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError>;
}

/// Deterministic test provider that maps input hashes to vectors.
#[derive(Debug, Clone)]
pub struct FakeEmbeddingProvider {
    specification: EmbeddingSpecification,
}

impl FakeEmbeddingProvider {
    pub fn new(specification: EmbeddingSpecification) -> Self {
        Self { specification }
    }

    fn embed_text(&self, text: &str) -> Result<EmbeddingVector, EmbeddingError> {
        let dims = self.specification.dimensions as usize;
        let digest = Sha256::digest(text.as_bytes());
        let mut values = Vec::with_capacity(dims);
        for index in 0..dims {
            let byte = digest[index % digest.len()];
            values.push((byte as f32 + 1.0) / 255.0);
        }
        if self.specification.normalized {
            normalize_l2(&mut values);
        }
        Ok(EmbeddingVector { values })
    }
}

#[async_trait]
impl EmbeddingProvider for FakeEmbeddingProvider {
    fn specification(&self) -> &EmbeddingSpecification {
        &self.specification
    }

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError> {
        self.embed_text(&request.text)
    }

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        requests
            .iter()
            .map(|request| self.embed_text(&request.text))
            .collect()
    }
}

fn normalize_l2(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specification::{DistanceMetric, PoolingStrategy};

    fn sample_spec(dims: u32) -> EmbeddingSpecification {
        EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:fake".into(),
            dimensions: dims,
            native_dimensions: dims,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Last,
            normalized: true,
            instruction_version: "test-v1".into(),
        }
    }

    #[tokio::test]
    async fn fake_provider_is_deterministic() {
        let provider = FakeEmbeddingProvider::new(sample_spec(8));
        let first = provider
            .embed_query(EmbedQueryRequest {
                text: "hello".into(),
            })
            .await
            .unwrap();
        let second = provider
            .embed_query(EmbedQueryRequest {
                text: "hello".into(),
            })
            .await
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.values.len(), 8);
    }

    #[tokio::test]
    async fn fake_provider_honors_dimensions() {
        let provider = FakeEmbeddingProvider::new(sample_spec(16));
        let vector = provider
            .embed_documents(vec![
                EmbedDocumentRequest {
                    chunk_id: "chunk-1".into(),
                    text: "doc".into(),
                },
                EmbedDocumentRequest {
                    chunk_id: "chunk-2".into(),
                    text: "other".into(),
                },
            ])
            .await
            .unwrap();
        assert_eq!(vector.len(), 2);
        assert_eq!(vector[0].values.len(), 16);
        assert_ne!(vector[0], vector[1]);
    }
}
