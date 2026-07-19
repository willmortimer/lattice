use std::path::{Path, PathBuf};

use lattice_embedding::EmbeddingProvider;
use lattice_index::{
    Backlink, ChunkSearchHit, EmbedPendingStats, EmbeddingNamespace, HybridSearchHit, SearchHit,
    WorkspaceIndex, CHUNKER_VERSION,
};

fn ensure_index(root: &Path) -> Result<WorkspaceIndex, String> {
    let index = WorkspaceIndex::open(root).map_err(|err| err.to_string())?;
    if index.resource_count().map_err(|err| err.to_string())? == 0 {
        index.rebuild(root).map_err(|err| err.to_string())?;
    }
    Ok(index)
}

/// Rebuild the search index for `root`.
pub fn rebuild_index(root: String) -> Result<u64, String> {
    let root = PathBuf::from(root);
    let index = WorkspaceIndex::open(&root).map_err(|err| err.to_string())?;
    let stats = index.rebuild(&root).map_err(|err| err.to_string())?;
    Ok(stats.pages_indexed as u64)
}

/// Full-text search over the workspace's indexed pages.
pub fn search_workspace(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index.search(&query, limit).map_err(|err| err.to_string())
}

/// Full-text search over structural chunks in the workspace index.
pub fn search_workspace_chunks(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<ChunkSearchHit>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index
        .search_chunks(&query, limit)
        .map_err(|err| err.to_string())
}

/// Hybrid chunk search. Without a provider this is FTS-only with hybrid hit shape.
pub fn hybrid_search_workspace(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<HybridSearchHit>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index
        .hybrid_search(&query, limit, None, None)
        .map_err(|err| err.to_string())
}

/// Hybrid chunk search with an embedding provider and registered namespace.
pub fn hybrid_search_workspace_with_provider(
    root: String,
    query: String,
    limit: usize,
    provider: &dyn EmbeddingProvider,
    namespace_id: i64,
) -> Result<Vec<HybridSearchHit>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index
        .hybrid_search(&query, limit, Some(provider), Some(namespace_id))
        .map_err(|err| err.to_string())
}

/// Register a namespace and embed pending chunks for semantic hybrid search.
pub fn embed_workspace_pending_chunks(
    root: String,
    provider: &dyn EmbeddingProvider,
    batch_size: usize,
) -> Result<(EmbeddingNamespace, EmbedPendingStats), String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    let namespace = index
        .register_embedding_namespace(provider.specification(), CHUNKER_VERSION)
        .map_err(|err| err.to_string())?;
    let stats = index
        .embed_pending_chunks(namespace.id, provider, batch_size)
        .map_err(|err| err.to_string())?;
    Ok((namespace, stats))
}

/// List resources that link to `rel_path`, for the backlinks footer.
pub fn get_backlinks(root: String, rel_path: String) -> Result<Vec<Backlink>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index
        .backlinks(Path::new(&rel_path))
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(hits.iter().all(|hit| hit.source_end_byte > hit.source_start_byte));
    }

    #[test]
    fn search_workspace_returns_no_hits_for_an_empty_workspace() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace(root, "anything".to_string(), 10).unwrap();
        assert!(hits.is_empty());
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
        assert!(hits.iter().any(|hit| hit.resource_uri.ends_with("Notes.md")));
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
