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
mod session;

pub use error::{Error, Result};
pub use events::{EventBus, RuntimeEvent};
pub use session::WorkspaceSession;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

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
    events: EventBus,
    /// Serializes open/close so concurrent opens of the same root share one session.
    open_lock: Mutex<()>,
}

impl LatticeRuntime {
    pub fn new() -> Self {
        Self {
            sessions_by_root: RwLock::new(HashMap::new()),
            sessions_by_id: RwLock::new(HashMap::new()),
            events: EventBus::new(),
            open_lock: Mutex::new(()),
        }
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    /// Open (or return) a warm session for `root`.
    ///
    /// The second open of the same canonical path returns the same
    /// [`Arc<WorkspaceSession>`]. The search index is opened once and kept warm.
    pub fn open_workspace_session(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<Arc<WorkspaceSession>> {
        let canonical = canonicalize_root(root.as_ref())?;
        {
            let by_root = self.sessions_by_root.read().expect("sessions poisoned");
            if let Some(existing) = by_root.get(&canonical) {
                return Ok(Arc::clone(existing));
            }
        }

        let _guard = self.open_lock.lock().expect("open lock poisoned");
        // Re-check after acquiring the open lock.
        {
            let by_root = self.sessions_by_root.read().expect("sessions poisoned");
            if let Some(existing) = by_root.get(&canonical) {
                return Ok(Arc::clone(existing));
            }
        }

        let session = Arc::new(WorkspaceSession::open(&canonical)?);
        let workspace_id = session.workspace_id().to_string();

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
    use lattice_core::Workspace;
    use std::sync::Barrier;
    use std::thread;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Runtime Test").unwrap();
        dir
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
}
