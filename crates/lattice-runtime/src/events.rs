use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

/// Lightweight fan-out bus for session lifecycle signals.
///
/// D1 keeps this synchronous and in-process. Daemon event streaming over the
/// wire arrives in later phases.
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
}
