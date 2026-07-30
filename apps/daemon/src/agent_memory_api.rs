//! Agent-memory HTTP API (`/v1/agent_memory/*`).
//!
//! Workspace-local Lance rows behind latticed; agents must not open Lance directly.
//! When the daemon semantic embedding provider is available (512-d Qwen/Pioneer),
//! remember/recall embed text server-side. Consent/retention policy is deferred
//! (ADR 0064 stub).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_embedding::{
    EmbedDocumentRequest, EmbedQueryRequest, EmbeddingProvider,
};
use lattice_lance::{
    AgentMemoryRecallRequest, AgentMemoryRecallResults, AgentMemoryRow, AgentMemoryStore,
    AGENT_MEMORY_EMBEDDING_WIDTH,
};
use lattice_runtime::LatticeRuntime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::{resolve_session, ApiError, MAX_HIT_LIMIT};
use crate::embed_host::SemanticController;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RememberParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RememberResponse {
    pub workspace_id: String,
    pub memory_id: String,
    pub version: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub query: String,
    #[serde(default)]
    pub query_embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecallResponse {
    pub workspace_id: String,
    pub hits: Vec<AgentMemoryHitDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryHitDto {
    pub memory_id: String,
    pub text: String,
    pub score: f32,
    pub metadata: Value,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMemoryParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMemoryResponse {
    pub workspace_id: String,
    pub deleted_count: usize,
    pub version: u64,
}

fn clamp_memory_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(10).clamp(1, MAX_HIT_LIMIT)
}

fn workspace_root_from_session(session: &lattice_runtime::WorkspaceSession) -> PathBuf {
    session.root().to_path_buf()
}

fn block_on_lance<F, T>(future: F) -> Result<T, ApiError>
where
    F: std::future::Future<Output = lattice_lance::Result<T>>,
{
    tokio::runtime::Handle::try_current()
        .map_err(|err| ApiError::Internal(err.to_string()))
        .and_then(|handle| {
            tokio::task::block_in_place(|| {
                handle
                    .block_on(future)
                    .map_err(|err| ApiError::Internal(err.to_string()))
            })
        })
}

fn metadata_json(metadata: Option<Value>) -> Result<String, ApiError> {
    match metadata {
        None => Ok("{}".to_string()),
        Some(value) => serde_json::to_string(&value)
            .map_err(|err| ApiError::BadRequest(format!("metadata must be JSON: {err}"))),
    }
}

fn parse_metadata_json(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or(Value::Object(Default::default()))
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Return the daemon embedding provider when it matches agent-memory width.
pub(crate) fn embedding_provider_for_agent_memory(
    provider: Option<Arc<dyn EmbeddingProvider>>,
) -> Option<Arc<dyn EmbeddingProvider>> {
    let provider = provider?;
    if provider.specification().dimensions != AGENT_MEMORY_EMBEDDING_WIDTH {
        return None;
    }
    Some(provider)
}

fn agent_memory_embedding_provider(
    semantic: Option<&SemanticController>,
) -> Option<Arc<dyn EmbeddingProvider>> {
    embedding_provider_for_agent_memory(semantic.and_then(|controller| controller.provider()))
}

fn block_on_embed<F, T>(future: F) -> Result<T, ApiError>
where
    F: std::future::Future<Output = Result<T, lattice_embedding::EmbeddingError>>,
{
    tokio::runtime::Handle::try_current()
        .map_err(|err| ApiError::Internal(err.to_string()))
        .and_then(|handle| {
            tokio::task::block_in_place(|| {
                handle
                    .block_on(future)
                    .map_err(|err| ApiError::Internal(err.to_string()))
            })
        })
}

fn try_embed_memory_text(
    provider: &Arc<dyn EmbeddingProvider>,
    memory_id: &str,
    text: &str,
) -> Option<Vec<f32>> {
    let provider = Arc::clone(provider);
    let memory_id = memory_id.to_string();
    let text = text.to_string();
    block_on_embed(async move {
        let mut vectors = provider
            .embed_documents(vec![EmbedDocumentRequest {
                chunk_id: memory_id,
                text,
            }])
            .await?;
        vectors.pop().ok_or_else(|| {
            lattice_embedding::EmbeddingError::provider("embed_documents returned no vector")
        })
    })
    .ok()
    .map(|vector| vector.values)
}

fn try_embed_query_text(provider: &Arc<dyn EmbeddingProvider>, query: &str) -> Option<Vec<f32>> {
    let provider = Arc::clone(provider);
    let query = query.to_string();
    block_on_embed(async move {
        provider
            .embed_query(EmbedQueryRequest { text: query })
            .await
    })
    .ok()
    .map(|vector| vector.values)
}

/// Upsert a workspace-local agent memory row.
pub fn api_remember(
    runtime: &LatticeRuntime,
    semantic: Option<&SemanticController>,
    params: RememberParams,
) -> Result<RememberResponse, ApiError> {
    if params.text.trim().is_empty() {
        return Err(ApiError::BadRequest("text must not be empty".into()));
    }

    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let workspace_id = session.workspace_id().to_string();
    let root = workspace_root_from_session(&session);
    let memory_id = params
        .id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let metadata_json = metadata_json(params.metadata)?;

    let now = current_time_ms();
    let mut embedding = params.embedding;
    if embedding.as_ref().is_none_or(|values| values.is_empty()) {
        if let Some(provider) = agent_memory_embedding_provider(semantic) {
            embedding = try_embed_memory_text(&provider, &memory_id, &params.text);
        }
    }

    let row = AgentMemoryRow {
        memory_id: memory_id.clone(),
        text: params.text,
        embedding,
        metadata_json,
        created_at_ms: now,
        updated_at_ms: now,
    };

    let commit = block_on_lance(async {
        let store = AgentMemoryStore::open(&root).await?;
        store.remember(row).await
    })?;

    Ok(RememberResponse {
        workspace_id,
        memory_id,
        version: commit.version,
    })
}

/// Recall workspace-local agent memories by text and/or embedding.
pub fn api_recall(
    runtime: &LatticeRuntime,
    semantic: Option<&SemanticController>,
    params: RecallParams,
) -> Result<RecallResponse, ApiError> {
    if params.query.trim().is_empty() && params.query_embedding.as_ref().is_none_or(|v| v.is_empty())
    {
        return Err(ApiError::BadRequest(
            "query or queryEmbedding is required".into(),
        ));
    }

    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let workspace_id = session.workspace_id().to_string();
    let root = workspace_root_from_session(&session);

    let mut query_embedding = params.query_embedding;
    if query_embedding.as_ref().is_none_or(|values| values.is_empty())
        && !params.query.trim().is_empty()
    {
        if let Some(provider) = agent_memory_embedding_provider(semantic) {
            query_embedding = try_embed_query_text(&provider, &params.query);
        }
    }

    let results: AgentMemoryRecallResults = block_on_lance(async {
        let store = AgentMemoryStore::open(&root).await?;
        store
            .recall(AgentMemoryRecallRequest {
                query: params.query,
                query_embedding,
                limit: clamp_memory_limit(params.limit),
            })
            .await
    })?;

    let hits = results
        .hits
        .into_iter()
        .map(|hit| AgentMemoryHitDto {
            memory_id: hit.memory_id,
            text: hit.text,
            score: hit.score,
            metadata: parse_metadata_json(&hit.metadata_json),
            created_at_ms: hit.created_at_ms,
            updated_at_ms: hit.updated_at_ms,
        })
        .collect();

    Ok(RecallResponse {
        workspace_id,
        hits,
    })
}

/// Delete workspace-local agent memory rows by id.
pub fn api_delete_memory(
    runtime: &LatticeRuntime,
    params: DeleteMemoryParams,
) -> Result<DeleteMemoryResponse, ApiError> {
    if params.ids.is_empty() {
        return Err(ApiError::BadRequest("ids must not be empty".into()));
    }

    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let workspace_id = session.workspace_id().to_string();
    let root = workspace_root_from_session(&session);
    let deleted_count = params.ids.len();

    let commit = block_on_lance(async {
        let store = AgentMemoryStore::open(&root).await?;
        store.delete(&params.ids).await
    })?;

    Ok(DeleteMemoryResponse {
        workspace_id,
        deleted_count,
        version: commit.version,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lattice_embedding::{
        DistanceMetric, EmbeddingProvider, EmbeddingSpecification, FakeEmbeddingProvider,
        PoolingStrategy,
    };
    use lattice_lance::AGENT_MEMORY_EMBEDDING_WIDTH;

    use super::*;

    fn fake_provider(dimensions: u32) -> Arc<dyn EmbeddingProvider> {
        Arc::new(FakeEmbeddingProvider::new(EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:fake".into(),
            dimensions,
            native_dimensions: dimensions,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Last,
            normalized: true,
            instruction_version: "test-v1".into(),
        }))
    }

    #[test]
    fn embedding_provider_requires_matching_width() {
        assert!(embedding_provider_for_agent_memory(Some(fake_provider(
            AGENT_MEMORY_EMBEDDING_WIDTH
        )))
        .is_some());
        assert!(embedding_provider_for_agent_memory(Some(fake_provider(12))).is_none());
        assert!(embedding_provider_for_agent_memory(None).is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn try_embed_memory_text_returns_vector_for_matching_provider() {
        let provider = fake_provider(AGENT_MEMORY_EMBEDDING_WIDTH);
        let vector = try_embed_memory_text(&provider, "mem-1", "user prefers dark mode")
            .expect("embedding");
        assert_eq!(vector.len(), AGENT_MEMORY_EMBEDDING_WIDTH as usize);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn try_embed_query_text_returns_vector_for_matching_provider() {
        let provider = fake_provider(AGENT_MEMORY_EMBEDDING_WIDTH);
        let vector = try_embed_query_text(&provider, "dark mode").expect("embedding");
        assert_eq!(vector.len(), AGENT_MEMORY_EMBEDDING_WIDTH as usize);
    }
}
