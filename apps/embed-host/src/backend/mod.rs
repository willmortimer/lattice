mod fake;
#[cfg(feature = "llama-cpp")]
mod llama;

use async_trait::async_trait;
use lattice_embedding::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider, EmbeddingSpecification,
    EmbeddingVector, ModelManifest,
};

use crate::error::EmbedHostError;

pub use fake::FakeBackend;

/// Runtime backend selected at host start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Fake,
    #[cfg(feature = "llama-cpp")]
    LlamaCpp,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fake => "fake",
            #[cfg(feature = "llama-cpp")]
            Self::LlamaCpp => "llama-cpp",
        }
    }

    /// Backends compiled into this binary (one per line for CLI `backends`).
    pub fn available() -> &'static [&'static str] {
        #[cfg(feature = "llama-cpp")]
        {
            &["fake", "llama-cpp"]
        }
        #[cfg(not(feature = "llama-cpp"))]
        {
            &["fake"]
        }
    }

    pub fn parse(value: &str) -> Result<Self, EmbedHostError> {
        match value {
            "fake" => Ok(Self::Fake),
            "llama" | "llama-cpp" | "llama.cpp" => {
                #[cfg(feature = "llama-cpp")]
                {
                    Ok(Self::LlamaCpp)
                }
                #[cfg(not(feature = "llama-cpp"))]
                {
                    Err(EmbedHostError::BackendUnavailable(
                        "llama-cpp feature is not enabled; rebuild with --features llama-cpp (see README)"
                            .into(),
                    ))
                }
            }
            other => Err(EmbedHostError::protocol(format!(
                "unknown backend '{other}' (expected fake{})",
                if cfg!(feature = "llama-cpp") {
                    " or llama-cpp"
                } else {
                    ""
                }
            ))),
        }
    }
}

/// Backend that can be loaded from a verified model directory.
#[async_trait]
pub trait EmbeddingBackend: EmbeddingProvider {
    #[allow(dead_code)]
    fn kind(&self) -> BackendKind;
}

/// Construct a backend for a verified manifest + artifact path.
pub fn open_backend(
    kind: BackendKind,
    manifest: &ModelManifest,
    artifact_path: &std::path::Path,
    dimensions: u32,
) -> Result<Box<dyn EmbeddingBackend>, EmbedHostError> {
    match kind {
        BackendKind::Fake => {
            let _ = artifact_path;
            Ok(Box::new(FakeBackend::from_manifest(manifest, dimensions)?))
        }
        #[cfg(feature = "llama-cpp")]
        BackendKind::LlamaCpp => Ok(Box::new(llama::LlamaCppBackend::open(
            manifest,
            artifact_path,
            dimensions,
        )?)),
    }
}

/// Object-safe wrapper used by the host session when a model is loaded.
pub struct LoadedBackend {
    inner: Box<dyn EmbeddingBackend>,
}

impl LoadedBackend {
    pub fn new(inner: Box<dyn EmbeddingBackend>) -> Self {
        Self { inner }
    }

    pub fn specification(&self) -> &EmbeddingSpecification {
        self.inner.specification()
    }

    pub async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbedHostError> {
        self.inner
            .embed_query(request)
            .await
            .map_err(EmbedHostError::Embedding)
    }

    pub async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbedHostError> {
        self.inner
            .embed_documents(requests)
            .await
            .map_err(EmbedHostError::Embedding)
    }
}
