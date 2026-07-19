//! Long-lived process runtime for Lattice workspace sessions.
//!
//! Phase D1 introduces [`LatticeRuntime`] so handlers and embedded clients can
//! reuse an open [`WorkspaceIndex`] (and related session state) across calls
//! instead of reopening SQLite and rebuilding per request.
//!
//! String-path handlers may use [`default_runtime`] as a process-global
//! singleton for compatibility. Prefer passing an explicit [`Arc<LatticeRuntime>`]
//! when constructing hosts (Tauri, bridge, tests).

mod error;
mod events;
mod idempotency;
mod index_apply;
mod lease;
mod session;
mod watch;

pub use error::{Error, Result};
pub use events::{
    EventBus, IndexProgressPhase, ResourceChangeKind, RuntimeEvent, RuntimeIndexProgress,
    RuntimeResourceChanged, SharedEventBus,
};
pub use idempotency::{IdempotencyCache, IdempotentOutcome, DEFAULT_IDEMPOTENCY_CAPACITY};
pub use index_apply::{apply_workspace_event_to_index, resource_for_event, IndexApplyOutcome};
pub use lease::{
    acquire_workspace_lease, clear_workspace_lease, is_process_alive, lease_is_stale, lease_path,
    read_workspace_lease, require_workspace_lease, rfc3339_utc, write_workspace_lease, LeaseClaim,
    WorkspaceLeaseFile, LEASE_RELATIVE_PATH, OWNER_EMBEDDED, OWNER_LATTICED,
};
pub use session::WorkspaceSession;
pub use watch::{default_watch_debounce, SessionIndexWatcher};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

/// Process-global runtime used by String-path handler wrappers.
///
/// Prefer constructing and passing an explicit [`Arc<LatticeRuntime>`] into
/// hosts. This singleton exists so existing `open_workspace(root: String)` /
/// `search_workspace(...)` signatures keep working without every Tauri
/// command taking a runtime handle yet.
pub fn default_runtime() -> Arc<LatticeRuntime> {
    static RUNTIME: OnceLock<Arc<LatticeRuntime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| Arc::new(LatticeRuntime::new()))
        .clone()
}

/// Registry of warm workspace sessions keyed by canonical root path.
pub struct LatticeRuntime {
    sessions_by_root: RwLock<HashMap<PathBuf, Arc<WorkspaceSession>>>,
    sessions_by_id: RwLock<HashMap<String, Arc<WorkspaceSession>>>,
    events: SharedEventBus,
    /// Serializes open/close so concurrent opens of the same root share one session.
    open_lock: Mutex<()>,
    /// Debounce used when auto-starting watchers for write sessions.
    watch_debounce: Duration,
}

impl LatticeRuntime {
    pub fn new() -> Self {
        Self {
            sessions_by_root: RwLock::new(HashMap::new()),
            sessions_by_id: RwLock::new(HashMap::new()),
            events: Arc::new(EventBus::new()),
            open_lock: Mutex::new(()),
            watch_debounce: default_watch_debounce(),
        }
    }

    /// Construct a runtime with a custom watcher debounce (tests use a short value).
    pub fn with_watch_debounce(debounce: Duration) -> Self {
        let mut runtime = Self::new();
        runtime.watch_debounce = debounce;
        runtime
    }

    pub fn events(&self) -> &SharedEventBus {
        &self.events
    }

    /// Open (or return) a warm session for `root` without acquiring a write lease.
    ///
    /// Prefer [`Self::open_workspace_session_for_write`] when the caller will
    /// mutate the workspace.
    pub fn open_workspace_session(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<Arc<WorkspaceSession>> {
        self.open_or_get_session(root.as_ref(), false)
    }

    /// Open a warm session and acquire the workspace write lease for `claim`.
    ///
    /// Starts the session index watcher so external file edits update FTS.
    /// Fails with [`Error::LeaseHeld`] when another live owner holds the lease
    /// (embedded XOR `latticed`).
    pub fn open_workspace_session_for_write(
        &self,
        root: impl AsRef<Path>,
        claim: &LeaseClaim,
    ) -> Result<(Arc<WorkspaceSession>, WorkspaceLeaseFile)> {
        let lease = acquire_workspace_lease(root.as_ref(), claim)?;
        let session = self.open_or_get_session(root.as_ref(), true)?;
        session.set_write_lease(claim.clone());
        Ok((session, lease))
    }

    /// Explicitly start watching an already-open session (idempotent restart).
    pub fn start_watch(&self, session: &Arc<WorkspaceSession>) -> Result<()> {
        session.start_watching(Arc::clone(&self.events), self.watch_debounce)
    }

    fn open_or_get_session(
        &self,
        root: &Path,
        start_watcher: bool,
    ) -> Result<Arc<WorkspaceSession>> {
        let canonical = canonicalize_root(root)?;
        {
            let by_root = self.sessions_by_root.read().expect("sessions poisoned");
            if let Some(existing) = by_root.get(&canonical) {
                if start_watcher && !existing.is_watching() {
                    // Existing read session upgraded to write: attach watcher.
                    let _ = existing.start_watching(Arc::clone(&self.events), self.watch_debounce);
                }
                return Ok(Arc::clone(existing));
            }
        }

        let _guard = self.open_lock.lock().expect("open lock poisoned");
        // Re-check after acquiring the open lock.
        {
            let by_root = self.sessions_by_root.read().expect("sessions poisoned");
            if let Some(existing) = by_root.get(&canonical) {
                if start_watcher && !existing.is_watching() {
                    let _ = existing.start_watching(Arc::clone(&self.events), self.watch_debounce);
                }
                return Ok(Arc::clone(existing));
            }
        }

        let session = Arc::new(WorkspaceSession::open(&canonical)?);
        let workspace_id = session.workspace_id().to_string();

        if start_watcher {
            // Non-fatal: workspace stays usable without live reconciliation.
            if let Err(err) =
                session.start_watching(Arc::clone(&self.events), self.watch_debounce)
            {
                eprintln!(
                    "lattice: failed to start workspace index watcher at {}: {err}",
                    canonical.display()
                );
            }
        }

        {
            let mut by_root = self.sessions_by_root.write().expect("sessions poisoned");
            let mut by_id = self.sessions_by_id.write().expect("sessions poisoned");
            by_root.insert(canonical.clone(), Arc::clone(&session));
            by_id.insert(workspace_id.clone(), Arc::clone(&session));
        }

        self.events.publish(RuntimeEvent::SessionOpened {
            root: canonical,
            workspace_id,
        });
        Ok(session)
    }

    pub fn get_session(&self, root: impl AsRef<Path>) -> Result<Option<Arc<WorkspaceSession>>> {
        let canonical = canonicalize_root(root.as_ref())?;
        let by_root = self.sessions_by_root.read().expect("sessions poisoned");
        Ok(by_root.get(&canonical).map(Arc::clone))
    }

    pub fn get_session_by_id(&self, workspace_id: &str) -> Option<Arc<WorkspaceSession>> {
        let by_id = self.sessions_by_id.read().expect("sessions poisoned");
        by_id.get(workspace_id).map(Arc::clone)
    }

    /// Drop the session for `root` if present. Returns whether a session was closed.
    pub fn close_session(&self, root: impl AsRef<Path>) -> Result<bool> {
        let canonical = canonicalize_root(root.as_ref())?;
        let _guard = self.open_lock.lock().expect("open lock poisoned");

        let removed = {
            let mut by_root = self.sessions_by_root.write().expect("sessions poisoned");
            by_root.remove(&canonical)
        };

        match removed {
            Some(session) => {
                session.stop_watching();
                let workspace_id = session.workspace_id().to_string();
                {
                    let mut by_id = self.sessions_by_id.write().expect("sessions poisoned");
                    by_id.remove(&workspace_id);
                }
                self.events.publish(RuntimeEvent::SessionClosed {
                    root: canonical,
                    workspace_id,
                });
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions_by_root
            .read()
            .expect("sessions poisoned")
            .len()
    }
}

impl Default for LatticeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn canonicalize_root(root: &Path) -> Result<PathBuf> {
    root.canonicalize()
        .map_err(|source| Error::Io {
            path: root.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::{Workspace, TEST_DEBOUNCE_TIMEOUT};
    use std::sync::Barrier;
    use std::thread;
    use std::time::{Duration, Instant};

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Runtime Test").unwrap();
        dir
    }

    fn recv_matching(
        rx: &std::sync::mpsc::Receiver<RuntimeEvent>,
        timeout: Duration,
        mut pred: impl FnMut(&RuntimeEvent) -> bool,
    ) -> Option<RuntimeEvent> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match rx.recv_timeout(remaining) {
                Ok(event) if pred(&event) => return Some(event),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    }

    #[test]
    fn open_workspace_session_returns_same_arc_on_second_open() {
        let dir = init_workspace();
        let runtime = LatticeRuntime::new();

        let first = runtime.open_workspace_session(dir.path()).unwrap();
        let second = runtime.open_workspace_session(dir.path()).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(runtime.session_count(), 1);
    }

    #[test]
    fn close_session_removes_registry_entry() {
        let dir = init_workspace();
        let runtime = LatticeRuntime::new();
        let session = runtime.open_workspace_session(dir.path()).unwrap();
        let id = session.workspace_id().to_string();

        assert!(runtime.close_session(dir.path()).unwrap());
        assert_eq!(runtime.session_count(), 0);
        assert!(runtime.get_session(dir.path()).unwrap().is_none());
        assert!(runtime.get_session_by_id(&id).is_none());
        assert!(!runtime.close_session(dir.path()).unwrap());
    }

    #[test]
    fn warm_index_is_reused_across_searches() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nWarm index text.\n").unwrap();
        let runtime = LatticeRuntime::new();
        let session = runtime.open_workspace_session(dir.path()).unwrap();

        let hits = session.search("Warm", 10).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
        let rebuilds_after_first = session.index_rebuild_count();
        assert!(rebuilds_after_first >= 1);

        let hits_again = session.search("Warm", 10).unwrap();
        assert!(hits_again.iter().any(|h| h.path.ends_with("Notes.md")));
        assert_eq!(
            session.index_rebuild_count(),
            rebuilds_after_first,
            "holding a session must not rebuild the index on every search"
        );
    }

    #[test]
    fn concurrent_reads_share_one_session() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Guide.md"),
            "# Intro\n\nConcurrent search target.\n",
        )
        .unwrap();
        let runtime = Arc::new(LatticeRuntime::new());
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let runtime = Arc::clone(&runtime);
                let barrier = Arc::clone(&barrier);
                let root = dir.path().to_path_buf();
                thread::spawn(move || {
                    barrier.wait();
                    let session = runtime.open_workspace_session(&root).unwrap();
                    session.search("Concurrent", 10).unwrap()
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(runtime.session_count(), 1);
        assert!(results
            .iter()
            .all(|hits| hits.iter().any(|h| h.path.ends_with("Guide.md"))));
    }

    #[test]
    fn open_for_write_acquires_lease() {
        let dir = init_workspace();
        let runtime = LatticeRuntime::new();
        let claim = LeaseClaim::embedded(std::process::id(), 11, 1, "emb");
        let (session, lease) = runtime
            .open_workspace_session_for_write(dir.path(), &claim)
            .unwrap();
        assert_eq!(lease.owner, OWNER_EMBEDDED);
        assert!(session.write_lease_claim().is_some());
        assert!(session.is_watching());
        let on_disk = read_workspace_lease(dir.path()).unwrap().unwrap();
        assert_eq!(on_disk.instance_id, "emb");
    }

    #[test]
    fn open_for_write_xor_blocks_second_owner() {
        let dir = init_workspace();
        let runtime = LatticeRuntime::new();
        // PID 1 (init/launchd) is alive on Unix and distinct from this process.
        write_workspace_lease(
            dir.path(),
            &WorkspaceLeaseFile {
                schema_version: 1,
                owner: OWNER_LATTICED.into(),
                pid: 1,
                process_start: 1,
                socket: "/tmp/latticed.sock".into(),
                protocol_version: 1,
                instance_id: "d1".into(),
                acquired_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        assert!(!lease_is_stale(
            &read_workspace_lease(dir.path()).unwrap().unwrap()
        ));

        let embedded = LeaseClaim::embedded(std::process::id(), 2, 1, "e1");
        let err = runtime
            .open_workspace_session_for_write(dir.path(), &embedded)
            .err()
            .expect("must fail");
        assert!(matches!(err, Error::LeaseHeld { .. }), "{err:?}");
    }

    #[test]
    fn write_session_watcher_incrementally_indexes_new_file() {
        let dir = init_workspace();
        let runtime = LatticeRuntime::with_watch_debounce(TEST_DEBOUNCE_TIMEOUT);
        let events = runtime.events().subscribe();
        let claim = LeaseClaim::embedded(std::process::id(), 11, 1, "watch-emb");
        let (session, _) = runtime
            .open_workspace_session_for_write(dir.path(), &claim)
            .unwrap();

        let started = recv_matching(&events, Duration::from_secs(2), |e| {
            matches!(
                e,
                RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                    phase: IndexProgressPhase::Started,
                    ..
                })
            )
        });
        assert!(started.is_some(), "expected IndexProgress::Started");

        std::fs::write(
            dir.path().join("Watched.md"),
            "# Watch\n\nunique-watcher-fts-token\n",
        )
        .unwrap();

        let progress = recv_matching(&events, Duration::from_secs(5), |e| {
            matches!(
                e,
                RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                    phase: IndexProgressPhase::Upserted,
                    path: Some(path),
                    ..
                }) if path.ends_with("Watched.md")
            )
        });
        assert!(progress.is_some(), "expected IndexProgress upsert for Watched.md");

        let changed = recv_matching(&events, Duration::from_secs(1), |e| {
            matches!(
                e,
                RuntimeEvent::ResourceChanged(RuntimeResourceChanged {
                    kind: ResourceChangeKind::Created,
                    path,
                    ..
                }) if path.ends_with("Watched.md")
            ) || matches!(
                e,
                RuntimeEvent::ResourceChanged(RuntimeResourceChanged {
                    kind: ResourceChangeKind::Modified,
                    path,
                    ..
                }) if path.ends_with("Watched.md")
            )
        });
        // ResourceChanged is published before IndexProgress; it may already
        // have been consumed while waiting for upsert. Accept either order.
        let _ = changed;

        let hits = session.search("unique-watcher-fts-token", 10).unwrap();
        assert!(
            hits.iter().any(|h| h.path.ends_with("Watched.md")),
            "incremental FTS upsert should make the new file searchable"
        );

        runtime.close_session(dir.path()).unwrap();
    }

    #[test]
    fn watcher_emits_sequenced_resource_and_index_events() {
        let dir = init_workspace();
        let runtime = LatticeRuntime::with_watch_debounce(TEST_DEBOUNCE_TIMEOUT);
        let events = runtime.events().subscribe();
        let claim = LeaseClaim::embedded(std::process::id(), 22, 1, "seq-emb");
        let (session, _) = runtime
            .open_workspace_session_for_write(dir.path(), &claim)
            .unwrap();
        assert!(session.is_watching());

        std::fs::write(dir.path().join("Seq.md"), "# Seq\n\nsequence-marker\n").unwrap();

        let mut saw_resource = false;
        let mut saw_index = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !(saw_resource && saw_index) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match events.recv_timeout(remaining) {
                Ok(RuntimeEvent::ResourceChanged(RuntimeResourceChanged { path, .. }))
                    if path.ends_with("Seq.md") =>
                {
                    saw_resource = true;
                }
                Ok(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                    phase: IndexProgressPhase::Upserted,
                    path: Some(path),
                    ..
                })) if path.ends_with("Seq.md") =>
                {
                    saw_index = true;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        assert!(saw_resource, "expected ResourceChanged for Seq.md");
        assert!(saw_index, "expected IndexProgress upsert for Seq.md");
    }
}
