use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

/// Lightweight fan-out bus for session lifecycle and index signals.
///
/// Synchronous and in-process. Daemon hosts bridge these into sequenced wire
/// [`lattice_protocol::Event`] frames.
#[derive(Debug, Default)]
pub struct EventBus {
    subscribers: Mutex<Vec<mpsc::Sender<RuntimeEvent>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self) -> mpsc::Receiver<RuntimeEvent> {
        let (tx, rx) = mpsc::channel();
        self.subscribers
            .lock()
            .expect("event bus poisoned")
            .push(tx);
        rx
    }

    pub fn publish(&self, event: RuntimeEvent) {
        let mut subscribers = self.subscribers.lock().expect("event bus poisoned");
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

/// Kind of filesystem resource change observed by the session watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
    RootDeleted,
}

impl ResourceChangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::RootDeleted => "root_deleted",
        }
    }
}

/// Incremental FTS maintenance phase for a single path (or watcher lifecycle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexProgressPhase {
    Started,
    Stopped,
    Upserted,
    Removed,
    Error,
}

impl IndexProgressPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Stopped => "stopped",
            Self::Upserted => "upserted",
            Self::Removed => "removed",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeResourceChanged {
    pub workspace_id: String,
    pub kind: ResourceChangeKind,
    pub path: PathBuf,
    pub revision: Option<String>,
    pub from_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIndexProgress {
    pub workspace_id: String,
    pub phase: IndexProgressPhase,
    pub path: Option<PathBuf>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    SessionOpened {
        root: PathBuf,
        workspace_id: String,
    },
    SessionClosed {
        root: PathBuf,
        workspace_id: String,
    },
    ResourceChanged(RuntimeResourceChanged),
    IndexProgress(RuntimeIndexProgress),
}

/// Shared handle used by long-lived hosts and watcher threads.
pub type SharedEventBus = Arc<EventBus>;
