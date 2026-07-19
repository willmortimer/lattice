//! Per-session filesystem watcher that incrementally updates the warm FTS index.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lattice_core::{WorkspaceEvent, WorkspaceWatcher, DEFAULT_DEBOUNCE_TIMEOUT};

use crate::events::{
    EventBus, IndexProgressPhase, ResourceChangeKind, RuntimeEvent, RuntimeIndexProgress,
    RuntimeResourceChanged,
};
use crate::index_apply::{apply_workspace_event_to_index, IndexApplyOutcome};
use crate::session::WorkspaceSession;
use crate::{Error, Result};

/// Handle for a running index watcher attached to a [`WorkspaceSession`].
pub struct SessionIndexWatcher {
    stop: Arc<AtomicBool>,
    watcher: Mutex<Option<WorkspaceWatcher>>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl SessionIndexWatcher {
    /// Stop the OS watch and wait for the apply thread to exit.
    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(watcher) = self.watcher.lock().expect("watcher poisoned").take() {
            watcher.stop();
        }
        if let Some(join) = self.join.lock().expect("join poisoned").take() {
            let _ = join.join();
        }
    }
}

/// Start watching `session`'s root and apply settled events to its warm index.
///
/// Publishes [`RuntimeEvent::ResourceChanged`] and [`RuntimeEvent::IndexProgress`]
/// on `events`. Failure to start the OS watch is returned; callers may treat it
/// as non-fatal for read-only usability.
///
/// The apply thread holds only a [`Weak`] session handle so dropping the last
/// strong [`Arc`] can stop the watcher without a reference cycle.
pub fn start_session_index_watcher(
    session: Arc<WorkspaceSession>,
    events: Arc<EventBus>,
    debounce: Duration,
) -> Result<SessionIndexWatcher> {
    let root = session.root().to_path_buf();
    let workspace_id = session.workspace_id().to_string();
    let (watcher, rx) = WorkspaceWatcher::start_with_debounce(root.clone(), debounce).map_err(
        |source| Error::Watch {
            path: root.clone(),
            source,
        },
    )?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let weak_session: Weak<WorkspaceSession> = Arc::downgrade(&session);
    let join = thread::Builder::new()
        .name(format!("lattice-index-watch-{}", workspace_id))
        .spawn(move || {
            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.clone(),
                phase: IndexProgressPhase::Started,
                path: None,
                detail: None,
            }));

            while !stop_flag.load(Ordering::Relaxed) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(event) => {
                        let Some(session) = weak_session.upgrade() else {
                            break;
                        };
                        dispatch_watch_event(&session, &events, &workspace_id, event);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if weak_session.strong_count() == 0 {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }

            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id,
                phase: IndexProgressPhase::Stopped,
                path: None,
                detail: None,
            }));
        })
        .map_err(|source| Error::Io {
            path: root,
            source,
        })?;

    Ok(SessionIndexWatcher {
        stop,
        watcher: Mutex::new(Some(watcher)),
        join: Mutex::new(Some(join)),
    })
}

/// Default production debounce for session watchers.
pub fn default_watch_debounce() -> Duration {
    DEFAULT_DEBOUNCE_TIMEOUT
}

fn dispatch_watch_event(
    session: &WorkspaceSession,
    events: &EventBus,
    workspace_id: &str,
    event: WorkspaceEvent,
) {
    if let Some(changed) = resource_changed_from_event(workspace_id, &event) {
        events.publish(RuntimeEvent::ResourceChanged(changed));
    }

    let outcome = apply_workspace_event_to_index(session.index(), &event);
    match outcome {
        IndexApplyOutcome::Ignored => {}
        IndexApplyOutcome::Upserted { path } => {
            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.to_string(),
                phase: IndexProgressPhase::Upserted,
                path: Some(path),
                detail: None,
            }));
        }
        IndexApplyOutcome::Removed { path } => {
            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.to_string(),
                phase: IndexProgressPhase::Removed,
                path: Some(path),
                detail: None,
            }));
        }
        IndexApplyOutcome::Renamed { from, to } => {
            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.to_string(),
                phase: IndexProgressPhase::Removed,
                path: Some(from),
                detail: None,
            }));
            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.to_string(),
                phase: IndexProgressPhase::Upserted,
                path: Some(to),
                detail: None,
            }));
        }
        IndexApplyOutcome::Skipped { path, reason } => {
            events.publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id: workspace_id.to_string(),
                phase: IndexProgressPhase::Error,
                path: Some(path),
                detail: Some(reason),
            }));
        }
    }
}

fn resource_changed_from_event(
    workspace_id: &str,
    event: &WorkspaceEvent,
) -> Option<RuntimeResourceChanged> {
    let workspace_id = workspace_id.to_string();
    match event {
        WorkspaceEvent::RootDeleted => Some(RuntimeResourceChanged {
            workspace_id,
            kind: ResourceChangeKind::RootDeleted,
            path: PathBuf::new(),
            revision: None,
            from_path: None,
        }),
        WorkspaceEvent::Created { path, revision } => Some(RuntimeResourceChanged {
            workspace_id,
            kind: ResourceChangeKind::Created,
            path: path.clone(),
            revision: Some(revision.clone()),
            from_path: None,
        }),
        WorkspaceEvent::Modified { path, revision } => Some(RuntimeResourceChanged {
            workspace_id,
            kind: ResourceChangeKind::Modified,
            path: path.clone(),
            revision: Some(revision.clone()),
            from_path: None,
        }),
        WorkspaceEvent::Deleted { path } => Some(RuntimeResourceChanged {
            workspace_id,
            kind: ResourceChangeKind::Deleted,
            path: path.clone(),
            revision: None,
            from_path: None,
        }),
        WorkspaceEvent::Renamed { from, to, revision } => Some(RuntimeResourceChanged {
            workspace_id,
            kind: ResourceChangeKind::Renamed,
            path: to.clone(),
            revision: Some(revision.clone()),
            from_path: Some(from.clone()),
        }),
    }
}
