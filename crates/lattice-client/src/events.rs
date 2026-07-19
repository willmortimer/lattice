use lattice_protocol::Event;
use tokio::sync::mpsc;

use crate::error::ClientError;

/// Subscription filter for daemon event streams.
///
/// D0 keeps this minimal; later phases add resource/job/voice selectors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventFilter {
    /// When set, only events for this workspace are delivered.
    pub workspace_id: Option<String>,
}

/// Bounded async stream of sequenced [`Event`] values.
#[derive(Debug)]
pub struct EventStream {
    receiver: mpsc::Receiver<Result<Event, ClientError>>,
}

impl EventStream {
    /// Wrap a receiver as an event stream.
    pub fn new(receiver: mpsc::Receiver<Result<Event, ClientError>>) -> Self {
        Self { receiver }
    }

    /// Create an immediately closed stream (no events).
    pub fn empty() -> Self {
        let (_tx, rx) = mpsc::channel(1);
        Self { receiver: rx }
    }

    /// Receive the next event, or `None` when the subscription ends.
    pub async fn next(&mut self) -> Option<Result<Event, ClientError>> {
        self.receiver.recv().await
    }
}
