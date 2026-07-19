use std::path::{Path, PathBuf};

use lattice_embedding::EmbeddingProvider;
use lattice_index::{
    Backlink, ChunkSearchHit, EmbedPendingStats, EmbeddingNamespace, HybridSearchHit, SearchHit,
    CHUNKER_VERSION,
};
use lattice_runtime::{default_runtime, LatticeRuntime, WorkspaceSession};

fn map_runtime_err(err: lattice_runtime::Error) -> String {
    err.to_string()
}

/// Rebuild the search index for `root` using the process-default runtime session.
pub fn rebuild_index(root: String) -> Result<u64, String> {
    rebuild_index_with_runtime(&default_runtime(), root)
}

pub fn rebuild_index_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
) -> Result<u64, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    rebuild_index_with_session(&session)
}

pub fn rebuild_index_with_session(session: &WorkspaceSession) -> Result<u64, String> {
    session.rebuild_index().map_err(map_runtime_err)
}

/// Full-text search over the workspace's indexed pages.
pub fn search_workspace(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    search_workspace_with_runtime(&default_runtime(), root, query, limit)
}

pub fn search_workspace_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    search_workspace_with_session(&session, &query, limit)
}

pub fn search_workspace_with_session(
    session: &WorkspaceSession,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    session.search(query, limit).map_err(map_runtime_err)
}

/// Full-text search over structural chunks in the workspace index.
pub fn search_workspace_chunks(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<ChunkSearchHit>, String> {
    search_workspace_chunks_with_runtime(&default_runtime(), root, query, limit)
}

pub fn search_workspace_chunks_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<ChunkSearchHit>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    search_workspace_chunks_with_session(&session, &query, limit)
}

pub fn search_workspace_chunks_with_session(
    session: &WorkspaceSession,
    query: &str,
    limit: usize,
) -> Result<Vec<ChunkSearchHit>, String> {
    session.search_chunks(query, limit).map_err(map_runtime_err)
}

/// Hybrid chunk search. Without a provider this is FTS-only with hybrid hit shape.
pub fn hybrid_search_workspace(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<HybridSearchHit>, String> {
    hybrid_search_workspace_with_runtime(&default_runtime(), root, query, limit)
}

pub fn hybrid_search_workspace_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<HybridSearchHit>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    hybrid_search_workspace_with_session(&session, &query, limit, None, None)
}

/// Hybrid chunk search with an embedding provider and registered namespace.
pub fn hybrid_search_workspace_with_provider(
    root: String,
    query: String,
    limit: usize,
    provider: &dyn EmbeddingProvider,
    namespace_id: i64,
) -> Result<Vec<HybridSearchHit>, String> {
    hybrid_search_workspace_with_runtime_and_provider(
        &default_runtime(),
        root,
        query,
        limit,
        provider,
        namespace_id,
    )
}

pub fn hybrid_search_workspace_with_runtime_and_provider(
    runtime: &LatticeRuntime,
    root: String,
    query: String,
    limit: usize,
    provider: &dyn EmbeddingProvider,
    namespace_id: i64,
) -> Result<Vec<HybridSearchHit>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    hybrid_search_workspace_with_session(
        &session,
        &query,
        limit,
        Some(provider),
        Some(namespace_id),
    )
}

pub fn hybrid_search_workspace_with_session(
    session: &WorkspaceSession,
    query: &str,
    limit: usize,
    provider: Option<&dyn EmbeddingProvider>,
    namespace_id: Option<i64>,
) -> Result<Vec<HybridSearchHit>, String> {
    session.ensure_index_warm().map_err(map_runtime_err)?;
    session
        .index()
        .hybrid_search(query, limit, provider, namespace_id)
        .map_err(|err| err.to_string())
}

/// Register a namespace and embed pending chunks for semantic hybrid search.
pub fn embed_workspace_pending_chunks(
    root: String,
    provider: &dyn EmbeddingProvider,
    batch_size: usize,
) -> Result<(EmbeddingNamespace, EmbedPendingStats), String> {
    embed_workspace_pending_chunks_with_runtime(
        &default_runtime(),
        root,
        provider,
        batch_size,
    )
}

pub fn embed_workspace_pending_chunks_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    provider: &dyn EmbeddingProvider,
    batch_size: usize,
) -> Result<(EmbeddingNamespace, EmbedPendingStats), String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    embed_workspace_pending_chunks_with_session(&session, provider, batch_size)
}

pub fn embed_workspace_pending_chunks_with_session(
    session: &WorkspaceSession,
    provider: &dyn EmbeddingProvider,
    batch_size: usize,
) -> Result<(EmbeddingNamespace, EmbedPendingStats), String> {
    session.ensure_index_warm().map_err(map_runtime_err)?;
    let namespace = session
        .index()
        .register_embedding_namespace(provider.specification(), CHUNKER_VERSION)
        .map_err(|err| err.to_string())?;
    let stats = session
        .index()
        .embed_pending_chunks(namespace.id, provider, batch_size)
        .map_err(|err| err.to_string())?;
    Ok((namespace, stats))
}

/// List resources that link to `rel_path`, for the backlinks footer.
pub fn get_backlinks(root: String, rel_path: String) -> Result<Vec<Backlink>, String> {
    get_backlinks_with_runtime(&default_runtime(), root, rel_path)
}

pub fn get_backlinks_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    rel_path: String,
) -> Result<Vec<Backlink>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    get_backlinks_with_session(&session, &rel_path)
}

pub fn get_backlinks_with_session(
    session: &WorkspaceSession,
    rel_path: &str,
) -> Result<Vec<Backlink>, String> {
    session
        .backlinks(Path::new(rel_path))
        .map_err(map_runtime_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use lattice_core::Workspace;
    use lattice_embedding::{
        DistanceMetric, EmbeddingSpecification, FakeEmbeddingProvider, PoolingStrategy,
    };

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn search_workspace_rebuilds_an_empty_index_and_finds_hits() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nSome welcome text.\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace(root, "welcome".to_string(), 10).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
    }

    #[test]
    fn get_backlinks_rebuilds_an_empty_index_and_finds_sources() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Home.md"), "See [[Target]].\n").unwrap();
        std::fs::write(dir.path().join("Target.md"), "# Target\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let backlinks = get_backlinks(root, "Target.md".to_string()).unwrap();
        assert!(backlinks.iter().any(|b| b.source_path.ends_with("Home.md")));
    }

    #[test]
    fn search_workspace_chunks_returns_structural_hits() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Guide.md"),
            "# Intro\n\nWelcome to structural chunks.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace_chunks(root, "structural".to_string(), 10).unwrap();
        assert!(hits.iter().any(|hit| hit.path.ends_with("Guide.md")));
        assert!(hits
            .iter()
            .any(|hit| hit.heading_path.contains(&"Intro".to_string())));
        assert!(hits
            .iter()
            .all(|hit| hit.source_end_byte > hit.source_start_byte));
    }

    #[test]
    fn search_workspace_returns_no_hits_for_an_empty_workspace() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace(root, "anything".to_string(), 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn search_with_session_reuses_warm_index() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nReusable warm text.\n").unwrap();
        let runtime = Arc::new(LatticeRuntime::new());
        let session = runtime.open_workspace_session(dir.path()).unwrap();

        let hits = search_workspace_with_session(&session, "Reusable", 10).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
        let rebuilds = session.index_rebuild_count();
        assert!(rebuilds >= 1);

        let hits_again = search_workspace_with_session(&session, "Reusable", 10).unwrap();
        assert!(hits_again.iter().any(|h| h.path.ends_with("Notes.md")));
        assert_eq!(session.index_rebuild_count(), rebuilds);
    }

    #[test]
    fn hybrid_search_workspace_fts_fallback_without_provider() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Hi\n\nCapability grants for plugins.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = hybrid_search_workspace(root, "capability".to_string(), 10).unwrap();
        assert!(hits
            .iter()
            .any(|hit| hit.resource_uri.ends_with("Notes.md")));
        assert!(hits.iter().all(|hit| hit.semantic_rank.is_none()));
    }

    #[test]
    fn hybrid_search_workspace_with_fake_embeddings() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Architecture.md"),
            "# Security\n\nPlugins execute outside the renderer with capability grants.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let provider = FakeEmbeddingProvider::new(EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:artifact".into(),
            dimensions: 12,
            native_dimensions: 12,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Last,
            normalized: true,
            instruction_version: "test-v1".into(),
        });
        let (namespace, stats) =
            embed_workspace_pending_chunks(root.clone(), &provider, 8).unwrap();
        assert!(stats.embedded > 0);

        let hits = hybrid_search_workspace_with_provider(
            root,
            "capability grants".to_string(),
            10,
            &provider,
            namespace.id,
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|hit| hit.fused_score > 0.0));
    }
}
