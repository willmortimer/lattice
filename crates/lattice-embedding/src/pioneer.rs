//! Pioneer OpenAI-compatible remote embeddings.
//!
//! Uses `POST https://api.pioneer.ai/v1/embeddings` with the same
//! `PIONEER_API_KEY` as the embedded agent. Vectors are requested at a fixed
//! Matryoshka-style output size (`dimensions`) and L2-normalized for cosine /
//! dot search in the Lattice index.
//!
//! This path is optional and selected via `LATTICE_EMBEDDING_PROVIDER=pioneer`.
//! Local Qwen / llama.cpp remains the default offline provider (ADR 0042).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::EmbeddingError;
use crate::provider::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider, EmbeddingVector,
};
use crate::specification::{DistanceMetric, EmbeddingSpecification, PoolingStrategy};

/// Env: select remote Pioneer embeddings instead of local embed-host.
pub const ENV_EMBEDDING_PROVIDER: &str = "LATTICE_EMBEDDING_PROVIDER";
/// Env: Pioneer API key (same secret as agentd; never sent to React).
pub const ENV_PIONEER_API_KEY: &str = "PIONEER_API_KEY";
/// Env: override embedding model id (default [`PIONEER_EMBEDDING_MODEL_ID`]).
pub const ENV_EMBEDDING_MODEL: &str = "LATTICE_EMBEDDING_MODEL";
/// Env: override output dimensions (default [`PIONEER_EMBEDDING_DIMENSIONS`]).
pub const ENV_EMBEDDING_DIMENSIONS: &str = "LATTICE_EMBEDDING_DIMENSIONS";

pub const PIONEER_EMBEDDING_BASE_URL: &str = "https://api.pioneer.ai/v1";
pub const PIONEER_EMBEDDING_MODEL_ID: &str = "text-embedding-3-small";
pub const PIONEER_EMBEDDING_MODEL_REVISION: &str = "pioneer-openai-compatible-v1";
pub const PIONEER_EMBEDDING_DIMENSIONS: u32 = 512;
pub const PIONEER_EMBEDDING_NATIVE_DIMENSIONS: u32 = 1536;
pub const PIONEER_EMBEDDING_INSTRUCTION_VERSION: &str = "pioneer-retrieval-v1";
pub const PIONEER_EMBEDDING_PROVIDER_ID: &str = "pioneer";

/// True when env requests Pioneer remote embeddings.
pub fn pioneer_embedding_requested() -> bool {
    match std::env::var(ENV_EMBEDDING_PROVIDER) {
        Ok(value) => value.trim().eq_ignore_ascii_case("pioneer"),
        Err(_) => false,
    }
}

/// Specification for the default Pioneer embedding namespace (512-d).
pub fn pioneer_embedding_specification(
    model_id: &str,
    dimensions: u32,
) -> EmbeddingSpecification {
    EmbeddingSpecification {
        provider_id: PIONEER_EMBEDDING_PROVIDER_ID.into(),
        model_id: model_id.into(),
        model_revision: PIONEER_EMBEDDING_MODEL_REVISION.into(),
        // Remote models have no local GGUF artifact; namespace key still needs a
        // stable placeholder distinct from local Qwen.
        artifact_sha256: format!("remote:{PIONEER_EMBEDDING_PROVIDER_ID}:{model_id}:{dimensions}"),
        dimensions,
        native_dimensions: PIONEER_EMBEDDING_NATIVE_DIMENSIONS,
        distance: DistanceMetric::Cosine,
        pooling: PoolingStrategy::Mean,
        normalized: true,
        instruction_version: PIONEER_EMBEDDING_INSTRUCTION_VERSION.into(),
    }
}

/// HTTP client for Pioneer `/v1/embeddings`.
#[derive(Debug, Clone)]
pub struct PioneerEmbeddingProvider {
    specification: EmbeddingSpecification,
    api_key: String,
    base_url: String,
}

impl PioneerEmbeddingProvider {
    /// Build from process environment.
    pub fn from_env() -> Result<Self, EmbeddingError> {
        let api_key = std::env::var(ENV_PIONEER_API_KEY)
            .map(|value| value.trim().to_string())
            .map_err(|_| {
                EmbeddingError::provider(format!(
                    "{ENV_PIONEER_API_KEY} is required when {ENV_EMBEDDING_PROVIDER}=pioneer"
                ))
            })?;
        if api_key.is_empty() {
            return Err(EmbeddingError::provider(format!(
                "{ENV_PIONEER_API_KEY} is empty"
            )));
        }
        let model_id = std::env::var(ENV_EMBEDDING_MODEL)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| PIONEER_EMBEDDING_MODEL_ID.into());
        let dimensions = std::env::var(ENV_EMBEDDING_DIMENSIONS)
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .unwrap_or(PIONEER_EMBEDDING_DIMENSIONS);
        if dimensions == 0 || dimensions > PIONEER_EMBEDDING_NATIVE_DIMENSIONS {
            return Err(EmbeddingError::InvalidDimensions {
                requested: dimensions,
                supported: PIONEER_EMBEDDING_NATIVE_DIMENSIONS,
            });
        }
        Ok(Self::new(api_key, model_id, dimensions, PIONEER_EMBEDDING_BASE_URL))
    }

    /// Construct with an explicit base URL (tests / alternate gateways).
    pub fn new(
        api_key: impl Into<String>,
        model_id: impl Into<String>,
        dimensions: u32,
        base_url: impl Into<String>,
    ) -> Self {
        let model_id = model_id.into();
        Self {
            specification: pioneer_embedding_specification(&model_id, dimensions),
            api_key: api_key.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn embed_texts(&self, inputs: &[String]) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let body = EmbeddingsRequest {
            model: self.specification.model_id.clone(),
            input: inputs.to_vec(),
            dimensions: Some(self.specification.dimensions),
        };
        let url = format!("{}/embeddings", self.base_url);
        let payload = serde_json::to_string(&body).map_err(|err| {
            EmbeddingError::provider(format!("pioneer embeddings encode: {err}"))
        })?;
        let response = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_string(&payload)
            .map_err(|err| EmbeddingError::provider(format!("pioneer embeddings request: {err}")))?;
        let status = response.status();
        let raw = response.into_string().map_err(|err| {
            EmbeddingError::provider(format!("pioneer embeddings read body ({status}): {err}"))
        })?;
        let parsed: EmbeddingsResponse = serde_json::from_str(&raw).map_err(|err| {
            EmbeddingError::provider(format!("pioneer embeddings decode ({status}): {err}"))
        })?;
        if status >= 400 {
            let message = parsed
                .error
                .as_ref()
                .and_then(|err| err.message.clone())
                .or(parsed.detail)
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(EmbeddingError::provider(format!(
                "pioneer embeddings failed: {message}"
            )));
        }
        parse_embeddings_response(&parsed, self.specification.dimensions, inputs.len())
    }
}

#[async_trait]
impl EmbeddingProvider for PioneerEmbeddingProvider {
    fn specification(&self) -> &EmbeddingSpecification {
        &self.specification
    }

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError> {
        let mut vectors = self.embed_texts(&[request.text])?;
        vectors.pop().ok_or_else(|| EmbeddingError::provider("empty embedding response"))
    }

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        let inputs: Vec<String> = requests.into_iter().map(|request| request.text).collect();
        self.embed_texts(&inputs)
    }
}

#[derive(Debug, Serialize)]
struct EmbeddingsRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    #[serde(default)]
    data: Vec<EmbeddingData>,
    #[serde(default)]
    error: Option<ApiErrorBody>,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: Option<String>,
}

fn parse_embeddings_response(
    parsed: &EmbeddingsResponse,
    expected_dims: u32,
    expected_count: usize,
) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
    if parsed.data.len() != expected_count {
        return Err(EmbeddingError::provider(format!(
            "expected {expected_count} embeddings, got {}",
            parsed.data.len()
        )));
    }
    let mut ordered = parsed.data.clone();
    ordered.sort_by_key(|item| item.index);
    let mut out = Vec::with_capacity(ordered.len());
    for item in ordered {
        if item.embedding.len() as u32 != expected_dims {
            return Err(EmbeddingError::InvalidDimensions {
                requested: item.embedding.len() as u32,
                supported: expected_dims,
            });
        }
        let mut values = item.embedding;
        normalize_l2(&mut values);
        out.push(EmbeddingVector { values });
    }
    Ok(out)
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

    #[test]
    fn parse_response_orders_by_index_and_normalizes() {
        let parsed = EmbeddingsResponse {
            data: vec![
                EmbeddingData {
                    index: 1,
                    embedding: vec![3.0, 4.0],
                },
                EmbeddingData {
                    index: 0,
                    embedding: vec![0.0, 2.0],
                },
            ],
            error: None,
            detail: None,
        };
        let vectors = parse_embeddings_response(&parsed, 2, 2).expect("parse");
        assert_eq!(vectors.len(), 2);
        assert!((vectors[0].values[1] - 1.0).abs() < 1e-5);
        assert!((vectors[1].values[0] - 0.6).abs() < 1e-5);
        assert!((vectors[1].values[1] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn specification_namespace_differs_from_fake() {
        let pioneer = pioneer_embedding_specification(PIONEER_EMBEDDING_MODEL_ID, 512)
            .namespace_key("lattice-chunker-v2");
        let fake = EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake".into(),
            model_revision: "r".into(),
            artifact_sha256: "sha256:x".into(),
            dimensions: 512,
            native_dimensions: 512,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Mean,
            normalized: true,
            instruction_version: "t".into(),
        }
        .namespace_key("lattice-chunker-v2");
        assert_ne!(pioneer, fake);
    }

    /// Live Pioneer probe (requires network + `PIONEER_API_KEY`).
    #[tokio::test]
    #[ignore = "requires PIONEER_API_KEY and network"]
    async fn live_pioneer_embed_query_and_documents() {
        let provider = PioneerEmbeddingProvider::from_env().expect("from_env");
        let query = provider
            .embed_query(EmbedQueryRequest {
                text: "lattice hybrid search".into(),
            })
            .await
            .expect("query");
        assert_eq!(
            query.values.len() as u32,
            provider.specification().dimensions
        );
        let docs = provider
            .embed_documents(vec![
                EmbedDocumentRequest {
                    chunk_id: "a".into(),
                    text: "first document".into(),
                },
                EmbedDocumentRequest {
                    chunk_id: "b".into(),
                    text: "second document".into(),
                },
            ])
            .await
            .expect("documents");
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].values.len(), query.values.len());
    }
}
