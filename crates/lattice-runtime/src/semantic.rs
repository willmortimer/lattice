//! Background semantic embedding jobs for warm workspace sessions.
//!
//! FTS remains usable while this worker catches up. Embedding work is
//! incremental via [`WorkspaceIndex::embed_pending_chunks`] (hash / stale
//! checks). Pause is a simple flag (memory/thermal hooks can set it later).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lattice_embedding::EmbeddingProvider;
use lattice_index::{EmbedPendingStats, HybridSearchHit, CHUNKER_VERSION};
use serde::{Deserialize, Serialize};

use crate::events::{
    EventBus, IndexProgressPhase, RuntimeEvent, RuntimeIndexProgress, SharedEventBus,
};
use crate::session::WorkspaceSession;
use crate::Result;

/// Default document batch size for background embedding.
pub const DEFAULT_EMBED_BATCH_SIZE: usize = 8;

/// How semantic search should behave when the provider/host is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticAvailability {
    Ready,
    Degraded,
    Paused,
    Stopped,
}

/// User-facing semantic session lifecycle for Settings / desktop status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticStatusState {
    Stopped,
    /// Explicit model download / verify (E5); never used during search.
    Downloading,
    Preparing,
    Indexing,
    Ready,
    Degraded,
    Failed,
}

impl SemanticStatusState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Downloading => "downloading",
            Self::Preparing => "preparing",
            Self::Indexing => "indexing",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "stopped" => Some(Self::Stopped),
            "downloading" => Some(Self::Downloading),
            "preparing" => Some(Self::Preparing),
            "indexing" => Some(Self::Indexing),
            "ready" => Some(Self::Ready),
            "degraded" => Some(Self::Degraded),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Snapshot returned by enable / disable / status queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStatus {
    pub state: SemanticStatusState,
    pub pending_chunks: Option<u64>,
    pub message: Option<String>,
    /// 0–100 while [`SemanticStatusState::Downloading`]; otherwise unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u32>,
}

impl SemanticStatus {
    pub fn stopped() -> Self {
        Self {
            state: SemanticStatusState::Stopped,
            pending_chunks: None,
            message: None,
            progress_percent: None,
        }
    }

    pub fn downloading(percent: u32) -> Self {
        let percent = percent.min(100);
        Self {
            state: SemanticStatusState::Downloading,
            pending_chunks: None,
            message: Some(format!("Downloading {percent}%")),
            progress_percent: Some(percent),
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerPhase {
    Preparing,
    Indexing,
    Idle,
    Failed,
}

/// Configuration for starting a session semantic worker.
pub struct SemanticWorkerConfig {
    pub provider: Arc<dyn EmbeddingProvider>,
    pub batch_size: usize,
    /// Optional dedicated Tokio runtime handle entered on the worker thread so
    /// host-backed providers can reuse IO bound to that runtime.
    pub runtime_handle: Option<tokio::runtime::Handle>,
}

impl SemanticWorkerConfig {
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self {
            provider,
            batch_size: DEFAULT_EMBED_BATCH_SIZE,
            runtime_handle: None,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    pub fn with_runtime_handle(mut self, handle: tokio::runtime::Handle) -> Self {
        self.runtime_handle = Some(handle);
        self
    }
}

struct SharedState {
    kick: Mutex<bool>,
    kick_cv: Condvar,
    paused: AtomicBool,
    degraded: AtomicBool,
    stop: AtomicBool,
    namespace_id: Mutex<Option<i64>>,
    phase: Mutex<WorkerPhase>,
    last_error: Mutex<Option<String>>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            kick: Mutex::new(false),
            kick_cv: Condvar::new(),
            paused: AtomicBool::new(false),
            degraded: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            namespace_id: Mutex::new(None),
            phase: Mutex::new(WorkerPhase::Preparing),
            last_error: Mutex::new(None),
        }
    }

    fn request_kick(&self) {
        let mut kick = self.kick.lock().expect("semantic kick poisoned");
        *kick = true;
        self.kick_cv.notify_one();
    }

    fn set_phase(&self, phase: WorkerPhase) {
        *self.phase.lock().expect("semantic phase poisoned") = phase;
    }

    fn set_error(&self, detail: String) {
        *self.last_error.lock().expect("semantic error poisoned") = Some(detail);
        self.set_phase(WorkerPhase::Failed);
    }

    fn wait_for_work(&self) -> bool {
        let mut kick = self.kick.lock().expect("semantic kick poisoned");
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return false;
            }
            if *kick && !self.paused.load(Ordering::SeqCst) && !self.degraded.load(Ordering::SeqCst)
            {
                *kick = false;
                return true;
            }
            let (guard, _) = self
                .kick_cv
                .wait_timeout(kick, Duration::from_millis(200))
                .expect("semantic wait poisoned");
            kick = guard;
        }
    }
}

/// Background worker that embeds pending chunks for one workspace session.
pub struct SessionSemanticWorker {
    shared: Arc<SharedState>,
    join: Mutex<Option<JoinHandle<()>>>,
    provider: Arc<dyn EmbeddingProvider>,
}

impl SessionSemanticWorker {
    /// Stop the worker and wait for the thread to exit.
    pub fn stop(self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.kick_cv.notify_all();
        if let Some(join) = self.join.lock().expect("semantic join poisoned").take() {
            let _ = join.join();
        }
    }

    pub fn kick(&self) {
        self.shared.request_kick();
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::SeqCst);
        self.shared.request_kick();
    }

    pub fn set_degraded(&self, degraded: bool) {
        let was = self.shared.degraded.swap(degraded, Ordering::SeqCst);
        if was && !degraded {
            self.shared.request_kick();
        }
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::SeqCst)
    }

    pub fn is_degraded(&self) -> bool {
        self.shared.degraded.load(Ordering::SeqCst)
    }

    pub fn namespace_id(&self) -> Option<i64> {
        *self
            .shared
            .namespace_id
            .lock()
            .expect("semantic namespace poisoned")
    }

    pub fn provider(&self) -> &Arc<dyn EmbeddingProvider> {
        &self.provider
    }

    pub fn availability(&self) -> SemanticAvailability {
        if self.shared.stop.load(Ordering::SeqCst) {
            SemanticAvailability::Stopped
        } else if self.shared.degraded.load(Ordering::SeqCst) {
            SemanticAvailability::Degraded
        } else if self.shared.paused.load(Ordering::SeqCst) {
            SemanticAvailability::Paused
        } else {
            SemanticAvailability::Ready
        }
    }

    /// Map worker flags + optional pending count into the Settings-facing status.
    pub fn status(&self, pending_chunks: Option<u64>) -> SemanticStatus {
        map_worker_status(
            self.shared.stop.load(Ordering::SeqCst),
            self.shared.degraded.load(Ordering::SeqCst),
            *self.shared.phase.lock().expect("semantic phase poisoned"),
            self.shared
                .last_error
                .lock()
                .expect("semantic error poisoned")
                .clone(),
            pending_chunks,
        )
    }
}

/// Pure status mapping for tests and hosts (daemon / Tauri).
pub fn map_worker_status(
    stopped: bool,
    degraded: bool,
    phase: impl Into<MappedWorkerPhase>,
    last_error: Option<String>,
    pending_chunks: Option<u64>,
) -> SemanticStatus {
    let phase = phase.into();
    if stopped {
        return SemanticStatus::stopped();
    }
    if degraded {
        return SemanticStatus {
            state: SemanticStatusState::Degraded,
            pending_chunks,
            message: last_error.or_else(|| Some("embed host unavailable".into())),
            progress_percent: None,
        };
    }
    match phase {
        MappedWorkerPhase::Preparing => SemanticStatus {
            state: SemanticStatusState::Preparing,
            pending_chunks,
            message: None,
            progress_percent: None,
        },
        MappedWorkerPhase::Indexing => SemanticStatus {
            state: SemanticStatusState::Indexing,
            pending_chunks,
            message: None,
            progress_percent: None,
        },
        MappedWorkerPhase::Idle => SemanticStatus {
            state: if pending_chunks.unwrap_or(0) > 0 {
                SemanticStatusState::Indexing
            } else {
                SemanticStatusState::Ready
            },
            pending_chunks,
            message: None,
            progress_percent: None,
        },
        MappedWorkerPhase::Failed => SemanticStatus {
            state: SemanticStatusState::Failed,
            pending_chunks,
            message: last_error,
            progress_percent: None,
        },
    }
}

/// Test/host-facing phase without exposing private [`WorkerPhase`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedWorkerPhase {
    Preparing,
    Indexing,
    Idle,
    Failed,
}

impl From<WorkerPhase> for MappedWorkerPhase {
    fn from(value: WorkerPhase) -> Self {
        match value {
            WorkerPhase::Preparing => Self::Preparing,
            WorkerPhase::Indexing => Self::Indexing,
            WorkerPhase::Idle => Self::Idle,
            WorkerPhase::Failed => Self::Failed,
        }
    }
}

/// Start a semantic embedding worker for `session`.
///
/// The worker registers an embedding namespace from `config.provider`, then
/// loops on kick signals calling [`WorkspaceIndex::embed_pending_chunks`].
pub fn start_session_semantic_worker(
    session: Arc<WorkspaceSession>,
    events: SharedEventBus,
    config: SemanticWorkerConfig,
) -> Result<SessionSemanticWorker> {
    let workspace_id = session.workspace_id().to_string();
    let shared = Arc::new(SharedState::new());
    let shared_thread = Arc::clone(&shared);
    let provider = Arc::clone(&config.provider);
    let provider_thread = Arc::clone(&config.provider);
    let batch_size = config.batch_size.max(1);
    let runtime_handle = config.runtime_handle;
    let weak_session: Weak<WorkspaceSession> = Arc::downgrade(&session);

    let join = thread::Builder::new()
        .name(format!("lattice-semantic-{}", workspace_id))
        .spawn(move || {
            // Keep host-backed providers on the runtime that owns their sockets.
            let _enter = runtime_handle.as_ref().map(|handle| handle.enter());

            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.clone(),
                phase: IndexProgressPhase::EmbeddingStarted,
                path: None,
                detail: None,
            }));

            let namespace_id = match weak_session.upgrade() {
                Some(session) => match session.ensure_index_warm() {
                    Ok(_) => match session.index().register_embedding_namespace(
                        provider_thread.specification(),
                        CHUNKER_VERSION,
                    ) {
                        Ok(ns) => {
                            *shared_thread
                                .namespace_id
                                .lock()
                                .expect("semantic namespace poisoned") = Some(ns.id);
                            shared_thread.set_phase(WorkerPhase::Idle);
                            Some(ns.id)
                        }
                        Err(err) => {
                            let detail = err.to_string();
                            shared_thread.set_error(detail.clone());
                            publish_error(&events, &workspace_id, detail);
                            None
                        }
                    },
                    Err(err) => {
                        let detail = err.to_string();
                        shared_thread.set_error(detail.clone());
                        publish_error(&events, &workspace_id, detail);
                        None
                    }
                },
                None => None,
            };

            if let Some(namespace_id) = namespace_id {
                // Initial catch-up for chunks already in the FTS index.
                shared_thread.request_kick();
                run_worker_loop(
                    &weak_session,
                    &events,
                    &workspace_id,
                    &shared_thread,
                    provider_thread.as_ref(),
                    namespace_id,
                    batch_size,
                );
            }

            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id,
                phase: IndexProgressPhase::EmbeddingIdle,
                path: None,
                detail: None,
            }));
        })
        .map_err(|source| crate::Error::Io {
            path: session.root().to_path_buf(),
            source,
        })?;

    Ok(SessionSemanticWorker {
        shared,
        join: Mutex::new(Some(join)),
        provider,
    })
}

fn run_worker_loop(
    weak_session: &Weak<WorkspaceSession>,
    events: &EventBus,
    workspace_id: &str,
    shared: &SharedState,
    provider: &dyn EmbeddingProvider,
    namespace_id: i64,
    batch_size: usize,
) {
    while shared.wait_for_work() {
        let Some(session) = weak_session.upgrade() else {
            break;
        };
        shared.set_phase(WorkerPhase::Indexing);
        match session
            .index()
            .embed_pending_chunks(namespace_id, provider, batch_size)
        {
            Ok(stats) => {
                publish_batch(events, workspace_id, &stats);
                // More pending work may remain if the batch was full.
                if stats.embedded > 0 || stats.failed > 0 {
                    if stats.embedded + stats.failed >= batch_size {
                        shared.request_kick();
                    } else {
                        shared.set_phase(WorkerPhase::Idle);
                    }
                } else {
                    shared.set_phase(WorkerPhase::Idle);
                }
            }
            Err(err) => {
                let detail = err.to_string();
                shared.set_error(detail.clone());
                publish_error(events, workspace_id, detail);
                // Back off briefly so a hard failure does not spin.
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn publish_batch(events: &EventBus, workspace_id: &str, stats: &EmbedPendingStats) {
    events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
        workspace_id: workspace_id.to_string(),
        phase: IndexProgressPhase::EmbeddingBatch,
        path: None,
        detail: Some(format!(
            "embedded={} skipped={} failed={}",
            stats.embedded, stats.skipped, stats.failed
        )),
    }));
}

fn publish_error(events: &EventBus, workspace_id: &str, detail: String) {
    events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
        workspace_id: workspace_id.to_string(),
        phase: IndexProgressPhase::Error,
        path: None,
        detail: Some(detail),
    }));
}

/// Hybrid search using the session's semantic worker when ready; otherwise FTS-only.
pub fn hybrid_search_with_session_semantic(
    session: &WorkspaceSession,
    query: &str,
    limit: usize,
) -> Result<Vec<HybridSearchHit>> {
    session.ensure_index_warm()?;
    match session.semantic_worker_snapshot() {
        Some((provider, namespace_id, avail))
            if matches!(
                avail,
                SemanticAvailability::Ready | SemanticAvailability::Paused
            ) =>
        {
            Ok(session.index().hybrid_search(
                query,
                limit,
                Some(provider.as_ref()),
                Some(namespace_id),
            )?)
        }
        _ => Ok(session.index().hybrid_search(query, limit, None, None)?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LatticeRuntime, LeaseClaim};
    use lattice_core::{Workspace, TEST_DEBOUNCE_TIMEOUT};
    use lattice_embedding::{
        DistanceMetric, EmbeddingSpecification, FakeEmbeddingProvider, PoolingStrategy,
    };
    use std::time::{Duration, Instant};

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Semantic Test").unwrap();
        dir
    }

    fn fake_provider() -> Arc<dyn EmbeddingProvider> {
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
            instruction_version: "test-v1".into(),
        }))
    }

    fn wait_for(
        rx: &std::sync::mpsc::Receiver<RuntimeEvent>,
        timeout: Duration,
        mut pred: impl FnMut(&RuntimeEvent) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(remaining) {
                Ok(event) if pred(&event) => return true,
                Ok(_) => continue,
                Err(_) => return false,
            }
        }
        false
    }

    #[test]
    fn file_change_eventually_embeds_and_hybrid_gets_semantic_ranks() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Seed.md"),
            "# Seed\n\ncapability grants for plugins.\n",
        )
        .unwrap();

        let runtime = LatticeRuntime::with_watch_debounce(TEST_DEBOUNCE_TIMEOUT);
        let events = runtime.events().subscribe();
        let claim = LeaseClaim::embedded(std::process::id(), 11, 1, "sem-emb");
        let (session, _) = runtime
            .open_workspace_session_for_write(dir.path(), &claim)
            .unwrap();

        session
            .start_semantic_indexing(
                Arc::clone(runtime.events()),
                SemanticWorkerConfig::new(fake_provider()),
            )
            .unwrap();

        assert!(wait_for(&events, Duration::from_secs(5), |e| {
            matches!(
                e,
                RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                    phase: IndexProgressPhase::EmbeddingBatch,
                    ..
                })
            )
        }));

        // Wait until the worker has a namespace and vectors.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut namespace_id = None;
        while Instant::now() < deadline {
            if let Some(id) = session.semantic_namespace_id() {
                let states = session
                    .index()
                    .chunk_embedding_states_for_namespace(id)
                    .unwrap();
                if states.iter().any(|s| s.status.as_str() == "ready") {
                    namespace_id = Some(id);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(namespace_id.is_some(), "expected embedded vectors");

        let hits = hybrid_search_with_session_semantic(&session, "capability grants", 10).unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits.iter().any(|hit| hit.semantic_rank.is_some()),
            "expected semantic ranks once vectors exist: {hits:?}"
        );

        std::fs::write(
            dir.path().join("New.md"),
            "# New\n\nunique-semantic-token-xyz\n",
        )
        .unwrap();

        assert!(wait_for(&events, Duration::from_secs(5), |e| {
            matches!(
                e,
                RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                    phase: IndexProgressPhase::Upserted,
                    path: Some(path),
                    ..
                }) if path.ends_with("New.md")
            )
        }));

        // Kick may already have been issued by the watcher; wait for another batch.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let fts = session.search("unique-semantic-token-xyz", 10).unwrap();
            if fts.iter().any(|h| h.path.ends_with("New.md")) {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        session.kick_semantic_jobs();
        let deadline = Instant::now() + Duration::from_secs(5);
        let ns = session.semantic_namespace_id().expect("namespace");
        while Instant::now() < deadline {
            let states = session
                .index()
                .chunk_embedding_states_for_namespace(ns)
                .unwrap();
            // New file should produce at least one ready chunk eventually.
            if states.len() >= 2 {
                break;
            }
            session.kick_semantic_jobs();
            thread::sleep(Duration::from_millis(25));
        }

        let hits =
            hybrid_search_with_session_semantic(&session, "unique-semantic-token-xyz", 10).unwrap();
        assert!(hits.iter().any(|hit| hit.resource_uri.ends_with("New.md")));

        runtime.close_session(dir.path()).unwrap();
    }

    #[test]
    fn degraded_provider_falls_back_to_fts_hybrid() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Notes\n\nCapability grants for plugins.\n",
        )
        .unwrap();
        let runtime = LatticeRuntime::new();
        let session = runtime.open_workspace_session(dir.path()).unwrap();
        session
            .start_semantic_indexing(
                Arc::clone(runtime.events()),
                SemanticWorkerConfig::new(fake_provider()),
            )
            .unwrap();

        // Let the worker register + embed once.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if session.semantic_namespace_id().is_some() {
                let ns = session.semantic_namespace_id().unwrap();
                let states = session
                    .index()
                    .chunk_embedding_states_for_namespace(ns)
                    .unwrap();
                if !states.is_empty() {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        session.set_semantic_degraded(true);
        assert_eq!(
            session.semantic_availability(),
            Some(SemanticAvailability::Degraded)
        );

        let hits = hybrid_search_with_session_semantic(&session, "capability", 10).unwrap();
        assert!(hits
            .iter()
            .any(|hit| hit.resource_uri.ends_with("Notes.md")));
        assert!(
            hits.iter().all(|hit| hit.semantic_rank.is_none()),
            "degraded mode must FTS-fallback without semantic ranks"
        );

        runtime.close_session(dir.path()).unwrap();
    }

    #[test]
    fn pause_and_resume_plus_restart_resumes_pending() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("A.md"), "# A\n\nalpha pending text\n").unwrap();
        let runtime = LatticeRuntime::new();
        let events = runtime.events().subscribe();
        let session = runtime.open_workspace_session(dir.path()).unwrap();

        session
            .start_semantic_indexing(
                Arc::clone(runtime.events()),
                SemanticWorkerConfig::new(fake_provider()).with_batch_size(1),
            )
            .unwrap();

        assert!(wait_for(&events, Duration::from_secs(5), |e| {
            matches!(
                e,
                RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                    phase: IndexProgressPhase::EmbeddingBatch,
                    ..
                })
            )
        }));

        session.pause_semantic_jobs();
        assert_eq!(
            session.semantic_availability(),
            Some(SemanticAvailability::Paused)
        );

        std::fs::write(dir.path().join("B.md"), "# B\n\nbeta pending text\n").unwrap();
        session.ensure_index_warm().unwrap();
        session
            .index()
            .upsert_page(std::path::Path::new("B.md"), "# B\n\nbeta pending text\n")
            .unwrap();
        session.kick_semantic_jobs();
        thread::sleep(Duration::from_millis(100));

        // Still paused: new chunk should remain pending/stale.
        let ns = session.semantic_namespace_id().expect("namespace");
        let before = session
            .index()
            .chunk_embedding_states_for_namespace(ns)
            .unwrap()
            .len();

        session.resume_semantic_jobs();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let after = session
                .index()
                .chunk_embedding_states_for_namespace(ns)
                .unwrap()
                .len();
            if after > before {
                break;
            }
            session.kick_semantic_jobs();
            thread::sleep(Duration::from_millis(25));
        }
        let after = session
            .index()
            .chunk_embedding_states_for_namespace(ns)
            .unwrap()
            .len();
        assert!(after > before, "resume should embed newly pending chunks");

        // Stop and restart worker: pending work should resume from state tables.
        session.stop_semantic_indexing();
        std::fs::write(dir.path().join("C.md"), "# C\n\ngamma pending text\n").unwrap();
        session
            .index()
            .upsert_page(std::path::Path::new("C.md"), "# C\n\ngamma pending text\n")
            .unwrap();

        session
            .start_semantic_indexing(
                Arc::clone(runtime.events()),
                SemanticWorkerConfig::new(fake_provider()).with_batch_size(4),
            )
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let states = session
                .index()
                .chunk_embedding_states_for_namespace(session.semantic_namespace_id().unwrap_or(ns))
                .unwrap();
            if states.len() >= after + 1 {
                break;
            }
            session.kick_semantic_jobs();
            thread::sleep(Duration::from_millis(25));
        }
        let final_count = session
            .index()
            .chunk_embedding_states_for_namespace(session.semantic_namespace_id().unwrap_or(ns))
            .unwrap()
            .len();
        assert!(
            final_count >= after + 1,
            "restart should resume pending chunks"
        );

        runtime.close_session(dir.path()).unwrap();
    }

    #[test]
    fn map_worker_status_covers_lifecycle_and_degrade() {
        assert_eq!(
            map_worker_status(true, false, MappedWorkerPhase::Idle, None, None).state,
            SemanticStatusState::Stopped
        );
        assert_eq!(
            map_worker_status(false, true, MappedWorkerPhase::Idle, None, Some(3)).state,
            SemanticStatusState::Degraded
        );
        assert_eq!(
            map_worker_status(false, false, MappedWorkerPhase::Preparing, None, None).state,
            SemanticStatusState::Preparing
        );
        assert_eq!(
            map_worker_status(false, false, MappedWorkerPhase::Indexing, None, Some(2)).state,
            SemanticStatusState::Indexing
        );
        assert_eq!(
            map_worker_status(false, false, MappedWorkerPhase::Idle, None, Some(0)).state,
            SemanticStatusState::Ready
        );
        assert_eq!(
            map_worker_status(false, false, MappedWorkerPhase::Idle, None, Some(4)).state,
            SemanticStatusState::Indexing
        );
        let failed = map_worker_status(
            false,
            false,
            MappedWorkerPhase::Failed,
            Some("boom".into()),
            None,
        );
        assert_eq!(failed.state, SemanticStatusState::Failed);
        assert_eq!(failed.message.as_deref(), Some("boom"));
        let downloading = SemanticStatus::downloading(42);
        assert_eq!(downloading.state, SemanticStatusState::Downloading);
        assert_eq!(downloading.progress_percent, Some(42));
        assert_eq!(downloading.message.as_deref(), Some("Downloading 42%"));
    }

    #[test]
    fn enable_path_status_leaves_stopped_then_returns_on_stop() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Notes\n\nCapability grants for plugins.\n",
        )
        .unwrap();
        let runtime = LatticeRuntime::new();
        let session = runtime.open_workspace_session(dir.path()).unwrap();
        assert_eq!(
            session.semantic_status().state,
            SemanticStatusState::Stopped
        );

        session
            .start_semantic_indexing(
                Arc::clone(runtime.events()),
                SemanticWorkerConfig::new(fake_provider()),
            )
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let status = session.semantic_status();
            if matches!(
                status.state,
                SemanticStatusState::Ready | SemanticStatusState::Indexing
            ) {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let status = session.semantic_status();
        assert!(
            matches!(
                status.state,
                SemanticStatusState::Ready
                    | SemanticStatusState::Indexing
                    | SemanticStatusState::Preparing
            ),
            "expected live status, got {status:?}"
        );

        session.set_semantic_degraded(true);
        assert_eq!(
            session.semantic_status().state,
            SemanticStatusState::Degraded
        );

        session.stop_semantic_indexing();
        assert_eq!(
            session.semantic_status().state,
            SemanticStatusState::Stopped
        );

        runtime.close_session(dir.path()).unwrap();
    }
}
