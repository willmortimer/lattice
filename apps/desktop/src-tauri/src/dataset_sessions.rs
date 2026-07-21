//! Process-local registry for cancellable dataset query / profile sessions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use lattice_arrow_transport::{AtomicCancel, CancelCheck};
use lattice_duckdb::InterruptHandle;

#[derive(Default)]
struct SessionEntry {
    cancelled: Arc<AtomicBool>,
    interrupt: Mutex<Option<Arc<InterruptHandle>>>,
}

#[derive(Default)]
struct SessionRegistry {
    sessions: Mutex<HashMap<String, Arc<SessionEntry>>>,
}

fn registry() -> &'static SessionRegistry {
    static REGISTRY: OnceLock<SessionRegistry> = OnceLock::new();
    REGISTRY.get_or_init(SessionRegistry::default)
}

/// Guard that unregisters a session when dropped (success or failure).
pub struct DatasetQuerySession {
    session_id: String,
    entry: Arc<SessionEntry>,
}

impl DatasetQuerySession {
    /// Register `session_id` for cooperative cancel + DuckDB interrupt.
    ///
    /// Replaces any prior entry with the same id so client retries stay safe.
    pub fn begin(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        let entry = Arc::new(SessionEntry::default());
        registry()
            .sessions
            .lock()
            .expect("dataset query session registry")
            .insert(session_id.clone(), Arc::clone(&entry));
        Self { session_id, entry }
    }

    pub fn cancel_token(&self) -> AtomicCancel {
        AtomicCancel::from_flag(Arc::clone(&self.entry.cancelled))
    }

    pub fn is_cancelled(&self) -> bool {
        self.entry.cancelled.load(Ordering::SeqCst)
    }

    /// Attach the DuckDB interrupt handle once the engine is open.
    pub fn bind_interrupt(&self, handle: Arc<InterruptHandle>) {
        *self
            .entry
            .interrupt
            .lock()
            .expect("dataset query interrupt lock") = Some(handle);
    }
}

impl CancelCheck for DatasetQuerySession {
    fn is_cancelled(&self) -> bool {
        self.entry.cancelled.load(Ordering::SeqCst)
    }
}

impl Drop for DatasetQuerySession {
    fn drop(&mut self) {
        let mut sessions = registry()
            .sessions
            .lock()
            .expect("dataset query session registry");
        if let Some(current) = sessions.get(&self.session_id) {
            if Arc::ptr_eq(current, &self.entry) {
                sessions.remove(&self.session_id);
            }
        }
    }
}

/// Flip the cancel flag and interrupt DuckDB if a live session exists.
///
/// Returns `true` when a session was found and signalled.
pub fn cancel_session(session_id: &str) -> bool {
    let entry = {
        let sessions = registry()
            .sessions
            .lock()
            .expect("dataset query session registry");
        sessions.get(session_id).map(Arc::clone)
    };
    let Some(entry) = entry else {
        return false;
    };
    entry.cancelled.store(true, Ordering::SeqCst);
    if let Some(handle) = entry
        .interrupt
        .lock()
        .expect("dataset query interrupt lock")
        .as_ref()
    {
        handle.interrupt();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn cancel_registry_marks_session_cancelled() {
        let session = DatasetQuerySession::begin("test-cancel-registry");
        assert!(!session.is_cancelled());
        assert!(cancel_session("test-cancel-registry"));
        assert!(session.is_cancelled());
        assert!(session.cancel_token().is_cancelled());
        drop(session);
        assert!(!cancel_session("test-cancel-registry"));
    }

    #[test]
    fn cancel_unknown_session_is_noop() {
        let started = Instant::now();
        assert!(!cancel_session("missing-session-id"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
