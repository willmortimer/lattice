//! Optional llama.cpp + Metal backend (`--features llama-cpp`).
//!
//! Loads a verified GGUF, runs embedding mode with last-token pooling, truncates
//! to the requested Matryoshka dimensions, then L2-normalizes. Real inference
//! tests are gated on `LATTICE_EMBED_LLAMA_GGUF` so CI stays offline.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;
use lattice_embedding::{
    DistanceMetric, EmbedDocumentRequest, EmbedQueryRequest, EmbeddingError, EmbeddingProvider,
    EmbeddingSpecification, EmbeddingVector, ModelManifest,
};
use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};

use super::{BackendKind, EmbeddingBackend};
use crate::error::EmbedHostError;

/// Query instruction template for `lattice-retrieval-v1` (docs/search).
const QUERY_INSTRUCTION: &str = "Instruct: Retrieve the most relevant passages, code, decisions, records, or notes from a private local workspace for answering the user's query.\nQuery: ";

fn shared_backend() -> Result<&'static LlamaBackend, EmbedHostError> {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    if let Some(backend) = BACKEND.get() {
        return Ok(backend);
    }
    let backend = LlamaBackend::init().map_err(|error| {
        EmbedHostError::BackendUnavailable(format!("llama.cpp backend init failed: {error}"))
    })?;
    let _ = BACKEND.set(backend);
    BACKEND.get().ok_or_else(|| {
        EmbedHostError::BackendUnavailable("llama.cpp backend missing after init".into())
    })
}

struct LlamaEngine {
    model: LlamaModel,
}

impl LlamaEngine {
    fn load(artifact_path: &Path) -> Result<Self, EmbedHostError> {
        let backend = shared_backend()?;
        // Offload all layers to Metal when the metal feature is linked.
        let model_params = LlamaModelParams::default().with_n_gpu_layers(1_000);
        let model = LlamaModel::load_from_file(backend, artifact_path, &model_params).map_err(
            |error| {
                EmbedHostError::BackendUnavailable(format!(
                    "failed to load GGUF {}: {error}",
                    artifact_path.display()
                ))
            },
        )?;
        Ok(Self { model })
    }

    fn embed_text(&self, text: &str, dimensions: u32) -> Result<EmbeddingVector, EmbeddingError> {
        let backend = shared_backend().map_err(|error| EmbeddingError::provider(error.to_string()))?;
        let n_ctx = NonZeroU32::new(2_048).expect("non-zero");
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(n_ctx))
            .with_embeddings(true)
            .with_pooling_type(LlamaPoolingType::Last);
        let mut ctx = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|error| EmbeddingError::provider(format!("llama context: {error}")))?;

        let tokens = self
            .model
            .str_to_token(text, AddBos::Always)
            .map_err(|error| EmbeddingError::provider(format!("tokenize: {error}")))?;
        if tokens.is_empty() {
            return Err(EmbeddingError::provider("empty token sequence"));
        }
        if tokens.len() as u32 > ctx.n_ctx() {
            return Err(EmbeddingError::provider(format!(
                "input exceeds context window ({} tokens > {})",
                tokens.len(),
                ctx.n_ctx()
            )));
        }

        let mut batch = LlamaBatch::new(tokens.len(), 1);
        batch
            .add_sequence(&tokens, 0, false)
            .map_err(|error| EmbeddingError::provider(format!("batch: {error}")))?;
        ctx.clear_kv_cache();
        ctx.decode(&mut batch)
            .map_err(|error| EmbeddingError::provider(format!("decode: {error}")))?;

        let embedding = ctx
            .embeddings_seq_ith(0)
            .map_err(|error| EmbeddingError::provider(format!("embeddings: {error}")))?;
        Ok(EmbeddingVector {
            values: matryoshka_l2(embedding, dimensions as usize)?,
        })
    }
}

/// Truncate to Matryoshka dims then L2-normalize (Lattice stored vectors).
fn matryoshka_l2(native: &[f32], dimensions: usize) -> Result<Vec<f32>, EmbeddingError> {
    if dimensions == 0 {
        return Err(EmbeddingError::provider("dimensions must be non-zero"));
    }
    if native.len() < dimensions {
        return Err(EmbeddingError::provider(format!(
            "model embedding length {} is smaller than requested dimensions {dimensions}",
            native.len()
        )));
    }
    let mut values = native[..dimensions].to_vec();
    normalize_l2(&mut values);
    Ok(values)
}

fn normalize_l2(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

fn format_query(text: &str) -> String {
    format!("{QUERY_INSTRUCTION}{text}")
}

/// llama.cpp GGUF embedding provider (Metal when linked).
pub struct LlamaCppBackend {
    specification: EmbeddingSpecification,
    engine: Mutex<LlamaEngine>,
    #[allow(dead_code)]
    artifact_path: PathBuf,
}

impl LlamaCppBackend {
    pub fn open(
        manifest: &ModelManifest,
        artifact_path: &Path,
        dimensions: u32,
    ) -> Result<Self, EmbedHostError> {
        if !artifact_path.is_file() {
            return Err(lattice_embedding::EmbeddingError::ArtifactNotFound {
                path: artifact_path.display().to_string(),
            }
            .into());
        }
        let mut specification =
            manifest.to_specification(dimensions, DistanceMetric::Cosine, true)?;
        specification.provider_id = "llama.cpp".into();

        let engine = LlamaEngine::load(artifact_path)?;
        Ok(Self {
            specification,
            engine: Mutex::new(engine),
            artifact_path: artifact_path.to_path_buf(),
        })
    }

    fn embed_blocking(&self, text: &str) -> Result<EmbeddingVector, EmbeddingError> {
        let dimensions = self.specification.dimensions;
        let engine = self
            .engine
            .lock()
            .map_err(|_| EmbeddingError::provider("llama engine lock poisoned"))?;
        engine.embed_text(text, dimensions)
    }
}

#[async_trait]
impl EmbeddingProvider for LlamaCppBackend {
    fn specification(&self) -> &EmbeddingSpecification {
        &self.specification
    }

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError> {
        let text = format_query(&request.text);
        self.embed_blocking(&text)
    }

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        let mut out = Vec::with_capacity(requests.len());
        for request in requests {
            out.push(self.embed_blocking(&request.text)?);
        }
        Ok(out)
    }
}

#[async_trait]
impl EmbeddingBackend for LlamaCppBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::LlamaCpp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matryoshka_truncates_and_normalizes() {
        let native = vec![3.0, 4.0, 0.0, 0.0];
        let values = matryoshka_l2(&native, 2).unwrap();
        assert_eq!(values.len(), 2);
        let norm = (values[0] * values[0] + values[1] * values[1]).sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
        assert!((values[0] - 0.6).abs() < 1e-5);
        assert!((values[1] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn matryoshka_rejects_oversized_request() {
        assert!(matryoshka_l2(&[1.0, 2.0], 4).is_err());
    }

    #[test]
    fn query_instruction_prefixes_user_text() {
        let formatted = format_query("hello");
        assert!(formatted.contains("Instruct:"));
        assert!(formatted.contains("Query: hello"));
    }
}
