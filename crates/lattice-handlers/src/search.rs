use std::path::{Path, PathBuf};
use std::sync::Arc;

use lattice_embedding::{
    acquire_pinned_embedding_model, download_progress_percent, semantic_fake_enabled,
    DistanceMetric, EmbeddingProvider, EmbeddingSpecification, FakeEmbeddingProvider,
    PoolingStrategy,
};
use lattice_index::{
    Backlink, CHUNKER_VERSION, ChunkSearchHit, EmbedPendingStats, EmbeddingNamespace,
    HybridSearchHit, SearchHit,
};
use lattice_runtime::{
    default_runtime, hybrid_search_with_session_semantic, LatticeRuntime, SemanticAvailability,
    SemanticStatus, SemanticStatusState, SemanticWorkerConfig, WorkspaceSession,
};
use serde::Serialize;

fn map_runtime_err(err: lattice_runtime::Error) -> String {
    err.to_string()
}

/// Search backend selection for the desktop / bridge IPC surface.
///
/// `None` / omitted mode parses as [`SearchMode::Fts`] so existing callers stay
/// on resource-level FTS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Resource-level FTS (`SearchHit` shape).
    Fts,
    /// Chunk hybrid search via the session semantic worker when ready; otherwise
    /// hybrid FTS-only fallback (no `semantic_rank`).
    Hybrid,
    /// Hybrid when the session semantic provider is ready/paused; otherwise FTS.
    /// Without a live semantic worker this matches FTS (not hybrid FTS fallback).
    Auto,
}

impl SearchMode {
    /// Parse an optional mode string. Empty / missing → [`SearchMode::Fts`].
    pub fn parse(mode: Option<&str>) -> Result<Self, String> {
        match mode.map(str::trim).filter(|s| !s.is_empty()) {
            None => Ok(Self::Fts),
            Some(raw) => match raw.to_ascii_lowercase().as_str() {
                "fts" => Ok(Self::Fts),
                "hybrid" => Ok(Self::Hybrid),
                "auto" => Ok(Self::Auto),
                other => Err(format!(
                    "unsupported search mode '{other}' (use fts, hybrid, or auto)"
                )),
            },
        }
    }
}

/// UI-facing search hit shared by FTS and hybrid IPC responses.
///
/// Base fields (`path`, `title`, `snippet`, `rank`) match the historical
/// `SearchHit` JSON shape so SearchPane callers keep working. Hybrid/auto
/// responses may include the optional chunk fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitUi {
    pub path: String,
    pub title: String,
    pub snippet: Option<String>,
    /// FTS BM25 rank, or hybrid fused score as `f64`.
    pub rank: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fused_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_path: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
}

impl SearchHitUi {
    pub(crate) fn from_fts(hit: SearchHit) -> Self {
        Self {
            path: hit.path.to_string_lossy().replace('\\', "/"),
            title: hit.title,
            snippet: hit.snippet,
            rank: hit.rank,
            fused_score: None,
            lexical_rank: None,
            semantic_rank: None,
            heading_path: None,
            chunk_id: None,
        }
    }

    pub(crate) fn from_hybrid(hit: HybridSearchHit) -> Self {
        let fused = f64::from(hit.fused_score);
        Self {
            path: path_from_resource_uri(&hit.resource_uri),
            title: hit.title,
            snippet: Some(hit.excerpt),
            rank: fused,
            fused_score: Some(fused),
            lexical_rank: hit.lexical_rank,
            semantic_rank: hit.semantic_rank,
            heading_path: Some(hit.heading_path),
            chunk_id: Some(hit.chunk_id),
        }
    }
}

fn path_from_resource_uri(uri: &str) -> String {
    uri.strip_prefix("lattice://resource/")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri)
        .replace('\\', "/")
}

fn session_semantic_ready(session: &WorkspaceSession) -> bool {
    matches!(
        session.semantic_availability(),
        Some(SemanticAvailability::Ready | SemanticAvailability::Paused)
    )
}

/// Rebuild the search index for `root` using the process-default runtime session.
pub fn rebuild_index(root: String) -> Result<u64, String> {
    rebuild_index_with_runtime(&default_runtime(), root)
}

pub fn rebuild_index_with_runtime(runtime: &LatticeRuntime, root: String) -> Result<u64, String> {
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

/// Mode-aware search returning the UI hit DTO used by desktop IPC.
///
/// `mode` omitted / `"fts"` keeps resource-level FTS behavior. `"hybrid"` always
/// runs hybrid search (session semantic when ready, else hybrid FTS fallback).
/// `"auto"` uses hybrid only when the session semantic provider is ready/paused;
/// otherwise it falls back to resource FTS.
pub fn search_workspace_ui(
    root: String,
    query: String,
    limit: usize,
    mode: Option<&str>,
) -> Result<Vec<SearchHitUi>, String> {
    search_workspace_ui_with_runtime(&default_runtime(), root, query, limit, mode)
}

pub fn search_workspace_ui_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
    query: String,
    limit: usize,
    mode: Option<&str>,
) -> Result<Vec<SearchHitUi>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    search_workspace_ui_with_session(&session, &query, limit, mode)
}

pub fn search_workspace_ui_with_session(
    session: &WorkspaceSession,
    query: &str,
    limit: usize,
    mode: Option<&str>,
) -> Result<Vec<SearchHitUi>, String> {
    let mode = SearchMode::parse(mode)?;
    match mode {
        SearchMode::Fts => search_workspace_with_session(session, query, limit)
            .map(|hits| hits.into_iter().map(SearchHitUi::from_fts).collect()),
        SearchMode::Hybrid => hybrid_search_with_session_semantic(session, query, limit)
            .map_err(|err| err.to_string())
            .map(|hits| hits.into_iter().map(SearchHitUi::from_hybrid).collect()),
        SearchMode::Auto => {
            if session_semantic_ready(session) {
                hybrid_search_with_session_semantic(session, query, limit)
                    .map_err(|err| err.to_string())
                    .map(|hits| hits.into_iter().map(SearchHitUi::from_hybrid).collect())
            } else {
                search_workspace_with_session(session, query, limit)
                    .map(|hits| hits.into_iter().map(SearchHitUi::from_fts).collect())
            }
        }
    }
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
    embed_workspace_pending_chunks_with_runtime(&default_runtime(), root, provider, batch_size)
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

fn fake_embedding_provider() -> Arc<dyn EmbeddingProvider> {
    Arc::new(FakeEmbeddingProvider::new(EmbeddingSpecification {
        provider_id: "fake".into(),
        model_id: "fake-model".into(),
        model_revision: "rev-1".into(),
        artifact_sha256: "sha256:fake".into(),
        dimensions: 12,
        native_dimensions: 12,
        distance: DistanceMetric::Cosine,
        pooling: PoolingStrategy::Last,
        normalized: true,
        instruction_version: "handlers-fake-v1".into(),
    }))
}

/// Start (or restart) semantic indexing for the workspace session.
///
/// When [`ENV_SEMANTIC_FAKE`] is set (CI / offline), skips model download and
/// uses the in-process Fake provider. Otherwise acquires the pinned Qwen3 GGUF
/// (local fixture via `LATTICE_SEMANTIC_MODEL_SOURCE`, or HTTPS) with progress
/// on the session prepare status, then starts the Fake worker until E6 wires
/// EmbedHostClient.
pub fn enable_semantic_search(root: String) -> Result<SemanticStatus, String> {
    enable_semantic_search_with_runtime(&default_runtime(), root)
}

pub fn enable_semantic_search_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
) -> Result<SemanticStatus, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    enable_semantic_search_with_session(runtime, &session)
}

pub fn enable_semantic_search_with_session(
    runtime: &LatticeRuntime,
    session: &Arc<WorkspaceSession>,
) -> Result<SemanticStatus, String> {
    enable_semantic_search_with_session_and_progress(runtime, session, |_| {})
}

/// Download / verify / install the pinned embedding model (or Fake-skip).
///
/// Progress is written to the session prepare status and forwarded to
/// `on_progress`. Safe to call from desktop handlers and latticed.
pub fn prepare_semantic_model_for_session(
    session: &WorkspaceSession,
    on_progress: &mut impl FnMut(&SemanticStatus),
) -> Result<(), String> {
    if semantic_fake_enabled() {
        session.set_semantic_prepare_status(None);
        return Ok(());
    }

    let mut last_percent = None;
    let acquire = acquire_pinned_embedding_model(&mut |copied, total| {
        let percent = download_progress_percent(copied, total);
        if last_percent == Some(percent) {
            return;
        }
        last_percent = Some(percent);
        let status = SemanticStatus::downloading(percent);
        session.set_semantic_prepare_status(Some(status.clone()));
        on_progress(&status);
    });
    match acquire {
        Ok(_result) => {
            session.set_semantic_prepare_status(Some(SemanticStatus {
                state: SemanticStatusState::Preparing,
                pending_chunks: None,
                message: Some("Model verified".into()),
                progress_percent: Some(100),
            }));
            let preparing = session.semantic_status();
            on_progress(&preparing);
            session.set_semantic_prepare_status(None);
            Ok(())
        }
        Err(error) => {
            let failed = SemanticStatus {
                state: SemanticStatusState::Failed,
                pending_chunks: None,
                message: Some(error.to_string()),
                progress_percent: None,
            };
            session.set_semantic_prepare_status(Some(failed.clone()));
            on_progress(&failed);
            Err(error.to_string())
        }
    }
}

/// Like [`enable_semantic_search_with_session`], with a progress hook for UI events.
pub fn enable_semantic_search_with_session_and_progress(
    runtime: &LatticeRuntime,
    session: &Arc<WorkspaceSession>,
    mut on_progress: impl FnMut(&SemanticStatus),
) -> Result<SemanticStatus, String> {
    prepare_semantic_model_for_session(session, &mut on_progress)?;
    session
        .start_semantic_indexing(
            Arc::clone(runtime.events()),
            SemanticWorkerConfig::new(fake_embedding_provider()),
        )
        .map_err(map_runtime_err)?;
    let status = session.semantic_status();
    on_progress(&status);
    Ok(status)
}

/// Stop semantic indexing for the workspace session (FTS remains available).
pub fn disable_semantic_search(root: String) -> Result<SemanticStatus, String> {
    disable_semantic_search_with_runtime(&default_runtime(), root)
}

pub fn disable_semantic_search_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
) -> Result<SemanticStatus, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    disable_semantic_search_with_session(&session)
}

pub fn disable_semantic_search_with_session(
    session: &WorkspaceSession,
) -> Result<SemanticStatus, String> {
    session.stop_semantic_indexing();
    Ok(SemanticStatus::stopped())
}

/// Current semantic indexing status for the workspace session.
pub fn semantic_search_status(root: String) -> Result<SemanticStatus, String> {
    semantic_search_status_with_runtime(&default_runtime(), root)
}

pub fn semantic_search_status_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
) -> Result<SemanticStatus, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    Ok(session.semantic_status())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    use lattice_core::Workspace;
    use lattice_embedding::{
        DistanceMetric, EmbeddingSpecification, FakeEmbeddingProvider, PoolingStrategy,
        ENV_SEMANTIC_FAKE,
    };
    use lattice_runtime::SemanticStatusState;

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
        assert!(
            hits.iter()
                .any(|hit| hit.heading_path.contains(&"Intro".to_string()))
        );
        assert!(
            hits.iter()
                .all(|hit| hit.source_end_byte > hit.source_start_byte)
        );
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
        assert!(
            hits.iter()
                .any(|hit| hit.resource_uri.ends_with("Notes.md"))
        );
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
        assert!(hits.iter().any(|hit| hit.semantic_rank.is_some()));
    }

    #[test]
    fn search_workspace_ui_fts_mode_matches_resource_hits() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nSome welcome text.\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let fts = search_workspace(root.clone(), "welcome".to_string(), 10).unwrap();
        let ui = search_workspace_ui(root, "welcome".to_string(), 10, Some("fts")).unwrap();
        assert_eq!(ui.len(), fts.len());
        assert!(ui.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(ui.iter().all(|h| h.chunk_id.is_none()));
        assert!(ui.iter().all(|h| h.fused_score.is_none()));
    }

    #[test]
    fn search_workspace_ui_omitted_mode_defaults_to_fts() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nDefault mode text.\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let ui = search_workspace_ui(root, "Default".to_string(), 10, None).unwrap();
        assert!(ui.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(ui.iter().all(|h| h.chunk_id.is_none()));
    }

    #[test]
    fn search_workspace_ui_auto_without_semantic_uses_fts() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Hi\n\nAuto fallback capability text.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let ui = search_workspace_ui(root, "capability".to_string(), 10, Some("auto")).unwrap();
        assert!(ui.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(ui.iter().all(|h| h.chunk_id.is_none()));
        assert!(ui.iter().all(|h| h.semantic_rank.is_none()));
    }

    #[test]
    fn search_workspace_ui_hybrid_without_provider_returns_chunk_hits() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Intro\n\nHybrid fallback grants text.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let ui = search_workspace_ui(root, "grants".to_string(), 10, Some("hybrid")).unwrap();
        assert!(ui.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(ui.iter().any(|h| h.chunk_id.is_some()));
        assert!(ui.iter().any(|h| h.fused_score.is_some()));
        assert!(ui.iter().all(|h| h.semantic_rank.is_none()));
    }

    #[test]
    fn search_workspace_ui_hybrid_with_fake_embeddings_sets_semantic_rank() {
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

        // Session-semantic hybrid needs a live worker; exercise the provider path
        // and map through the UI DTO the IPC layer uses.
        let raw = hybrid_search_workspace_with_provider(
            root,
            "capability grants".to_string(),
            10,
            &provider,
            namespace.id,
        )
        .unwrap();
        let ui: Vec<SearchHitUi> = raw.into_iter().map(SearchHitUi::from_hybrid).collect();
        assert!(!ui.is_empty());
        assert!(ui.iter().any(|hit| hit.semantic_rank.is_some()));
        assert!(
            ui.iter()
                .any(|hit| hit.fused_score.is_some_and(|s| s > 0.0))
        );
        assert!(ui.iter().any(|hit| hit.chunk_id.is_some()));
    }

    #[test]
    fn search_mode_rejects_unknown_values() {
        assert!(SearchMode::parse(Some("vector")).is_err());
        assert_eq!(SearchMode::parse(None).unwrap(), SearchMode::Fts);
        assert_eq!(SearchMode::parse(Some("AUTO")).unwrap(), SearchMode::Auto);
    }

    #[test]
    fn enable_disable_semantic_search_updates_status() {
        std::env::set_var(ENV_SEMANTIC_FAKE, "1");
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Notes\n\nCapability grants for plugins.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        assert_eq!(
            semantic_search_status(root.clone()).unwrap().state,
            SemanticStatusState::Stopped
        );

        let enabled = enable_semantic_search(root.clone()).unwrap();
        assert_ne!(enabled.state, SemanticStatusState::Stopped);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let status = semantic_search_status(root.clone()).unwrap();
            if matches!(
                status.state,
                SemanticStatusState::Ready | SemanticStatusState::Indexing
            ) {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        let disabled = disable_semantic_search(root.clone()).unwrap();
        assert_eq!(disabled.state, SemanticStatusState::Stopped);
        assert_eq!(
            semantic_search_status(root).unwrap().state,
            SemanticStatusState::Stopped
        );
        std::env::remove_var(ENV_SEMANTIC_FAKE);
    }
}
