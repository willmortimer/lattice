use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lattice_commands::CommandEngine;
use lattice_core::{ResourceCatalog, Workspace};
use lattice_embedding::EmbeddingProvider;
use lattice_index::{Backlink, ChunkSearchHit, SearchHit, WorkspaceIndex};

use crate::events::SharedEventBus;
use crate::idempotency::IdempotencyCache;
use crate::lease::{LeaseClaim, WorkspaceLeaseFile};
use crate::semantic::{
    start_session_semantic_worker, SemanticAvailability, SemanticWorkerConfig,
    SessionSemanticWorker,
};
use crate::watch::{default_watch_debounce, start_session_index_watcher, SessionIndexWatcher};
use crate::Result;

/// Long-lived open workspace: warm index, command engine, and optional catalog.
pub struct WorkspaceSession {
    root: PathBuf,
    workspace: Workspace,
    command_engine: Mutex<CommandEngine>,
    index: WorkspaceIndex,
    catalog: Mutex<Option<ResourceCatalog>>,
    index_rebuild_count: AtomicU64,
    /// Lease claim used when this session was opened for write, if any.
    write_lease: Mutex<Option<LeaseClaim>>,
    /// Recent mutation outcomes keyed by caller idempotency key.
    idempotency: IdempotencyCache,
    /// Active filesystem watcher that incrementally maintains the warm index.
    index_watcher: Mutex<Option<SessionIndexWatcher>>,
    /// Optional background semantic embedding worker.
    semantic_worker: Mutex<Option<SessionSemanticWorker>>,
}

impl WorkspaceSession {
    pub(crate) fn open(canonical_root: &Path) -> Result<Self> {
        let workspace = Workspace::open(canonical_root)?;
        let command_engine = CommandEngine::open(canonical_root)?;
        let index = WorkspaceIndex::open(canonical_root)?;
        let catalog = match workspace.scan() {
            Ok(resources) => Some(ResourceCatalog::new(&resources)),
            Err(_) => None,
        };
        Ok(Self {
            root: canonical_root.to_path_buf(),
            workspace,
            command_engine: Mutex::new(command_engine),
            index,
            catalog: Mutex::new(catalog),
            index_rebuild_count: AtomicU64::new(0),
            write_lease: Mutex::new(None),
            idempotency: IdempotencyCache::default(),
            index_watcher: Mutex::new(None),
            semantic_worker: Mutex::new(None),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace.manifest().id
    }

    pub fn index(&self) -> &WorkspaceIndex {
        &self.index
    }

    pub fn index_rebuild_count(&self) -> u64 {
        self.index_rebuild_count.load(Ordering::Relaxed)
    }

    pub fn with_command_engine<R>(&self, f: impl FnOnce(&mut CommandEngine) -> R) -> R {
        let mut engine = self.command_engine.lock().expect("command engine poisoned");
        f(&mut engine)
    }

    pub fn with_resource_catalog<R>(&self, f: impl FnOnce(Option<&ResourceCatalog>) -> R) -> R {
        let catalog = self.catalog.lock().expect("catalog poisoned");
        f(catalog.as_ref())
    }

    /// Ensure the warm index has at least one rebuild when empty.
    ///
    /// Returns `true` when this call performed a rebuild.
    pub fn ensure_index_warm(&self) -> Result<bool> {
        if self.index.resource_count()? == 0 {
            self.index.rebuild(&self.root)?;
            self.index_rebuild_count.fetch_add(1, Ordering::Relaxed);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn rebuild_index(&self) -> Result<u64> {
        let stats = self.index.rebuild(&self.root)?;
        self.index_rebuild_count.fetch_add(1, Ordering::Relaxed);
        Ok(stats.pages_indexed as u64)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        self.ensure_index_warm()?;
        Ok(self.index.search(query, limit)?)
    }

    pub fn search_chunks(&self, query: &str, limit: usize) -> Result<Vec<ChunkSearchHit>> {
        self.ensure_index_warm()?;
        Ok(self.index.search_chunks(query, limit)?)
    }

    pub fn backlinks(&self, rel_path: &Path) -> Result<Vec<Backlink>> {
        self.ensure_index_warm()?;
        Ok(self.index.backlinks(rel_path)?)
    }

    /// Record the lease claim that owns writes for this session.
    pub fn set_write_lease(&self, claim: LeaseClaim) {
        let mut guard = self.write_lease.lock().expect("write lease poisoned");
        *guard = Some(claim);
    }

    /// Lease claim recorded by the last successful open-for-write, if any.
    pub fn write_lease_claim(&self) -> Option<LeaseClaim> {
        self.write_lease
            .lock()
            .expect("write lease poisoned")
            .clone()
    }

    /// Clear the in-memory write-lease claim (does not delete the on-disk file).
    pub fn clear_write_lease(&self) {
        let mut guard = self.write_lease.lock().expect("write lease poisoned");
        *guard = None;
    }

    pub fn idempotency(&self) -> &IdempotencyCache {
        &self.idempotency
    }

    /// Re-read the on-disk lease when this session holds a write claim.
    pub fn current_lease_file(&self) -> Result<Option<WorkspaceLeaseFile>> {
        crate::lease::read_workspace_lease(&self.root)
    }

    /// Whether an index watcher is currently attached.
    pub fn is_watching(&self) -> bool {
        self.index_watcher
            .lock()
            .expect("index watcher poisoned")
            .is_some()
    }

    /// Start (or restart) the filesystem watcher that incrementally updates FTS.
    ///
    /// Intended for write sessions and daemon ownership of index maintenance.
    pub fn start_watching(
        self: &Arc<Self>,
        events: SharedEventBus,
        debounce: Duration,
    ) -> Result<()> {
        self.stop_watching();
        let watcher = start_session_index_watcher(Arc::clone(self), events, debounce)?;
        *self.index_watcher.lock().expect("index watcher poisoned") = Some(watcher);
        Ok(())
    }

    /// Start watching with the production debounce interval.
    pub fn start_watching_default(self: &Arc<Self>, events: SharedEventBus) -> Result<()> {
        self.start_watching(events, default_watch_debounce())
    }

    /// Stop the index watcher if running.
    pub fn stop_watching(&self) {
        if let Some(watcher) = self
            .index_watcher
            .lock()
            .expect("index watcher poisoned")
            .take()
        {
            watcher.stop();
        }
    }

    /// Start (or restart) background semantic embedding for this session.
    pub fn start_semantic_indexing(
        self: &Arc<Self>,
        events: SharedEventBus,
        config: SemanticWorkerConfig,
    ) -> Result<()> {
        self.stop_semantic_indexing();
        let worker = start_session_semantic_worker(Arc::clone(self), events, config)?;
        *self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned") = Some(worker);
        Ok(())
    }

    /// Stop the semantic embedding worker if running.
    pub fn stop_semantic_indexing(&self) {
        if let Some(worker) = self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .take()
        {
            worker.stop();
        }
    }

    pub fn is_semantic_indexing(&self) -> bool {
        self.semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .is_some()
    }

    /// Enqueue a catch-up pass over pending/stale chunks (no-op if no worker).
    pub fn kick_semantic_jobs(&self) {
        if let Some(worker) = self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .as_ref()
        {
            worker.kick();
        }
    }

    pub fn pause_semantic_jobs(&self) {
        if let Some(worker) = self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .as_ref()
        {
            worker.pause();
        }
    }

    pub fn resume_semantic_jobs(&self) {
        if let Some(worker) = self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .as_ref()
        {
            worker.resume();
        }
    }

    pub fn set_semantic_degraded(&self, degraded: bool) {
        if let Some(worker) = self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .as_ref()
        {
            worker.set_degraded(degraded);
        }
    }

    pub fn semantic_availability(&self) -> Option<SemanticAvailability> {
        self.semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .as_ref()
            .map(SessionSemanticWorker::availability)
    }

    pub fn semantic_namespace_id(&self) -> Option<i64> {
        self.semantic_worker
            .lock()
            .expect("semantic worker poisoned")
            .as_ref()
            .and_then(SessionSemanticWorker::namespace_id)
    }

    /// Snapshot of provider + namespace for hybrid search, when the worker is live.
    pub(crate) fn semantic_worker_snapshot(
        &self,
    ) -> Option<(Arc<dyn EmbeddingProvider>, i64, SemanticAvailability)> {
        let guard = self
            .semantic_worker
            .lock()
            .expect("semantic worker poisoned");
        let worker = guard.as_ref()?;
        let namespace_id = worker.namespace_id()?;
        Some((
            Arc::clone(worker.provider()),
            namespace_id,
            worker.availability(),
        ))
    }
}

impl Drop for WorkspaceSession {
    fn drop(&mut self) {
        self.stop_semantic_indexing();
        self.stop_watching();
    }
}
