use lattice_protocol::{event, Event};
use tokio::sync::mpsc;

use crate::error::ClientError;

/// Subscription filter for daemon event streams.
///
/// D0 keeps this minimal; later phases add resource/job/voice selectors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventFilter {
    /// When set, only events for this workspace are delivered.
    pub workspace_id: Option<String>,
    /// When true, only `AgentEvent` bodies are delivered.
    ///
    /// Agent runs share the workspace event bus with index/resource chatter; without
    /// this filter a busy workspace can Lagged-drop the tool/result chunks the UI awaits.
    pub agent_events_only: bool,
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

pub(crate) fn event_matches_filter(event: &Event, filter: &EventFilter) -> bool {
    if let Some(workspace_id) = filter.workspace_id.as_ref() {
        if &event.workspace_id != workspace_id {
            return false;
        }
    }
    if filter.agent_events_only {
        return matches!(event.body, Some(event::Body::AgentEvent(_)));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_protocol::AgentEvent;

    #[test]
    fn agent_events_only_skips_non_agent_bodies() {
        let agent = Event {
            sequence: 1,
            workspace_id: "ws".into(),
            body: Some(event::Body::AgentEvent(AgentEvent {
                run_id: "r1".into(),
                event_type: "run_started".into(),
                payload_json: "{}".into(),
            })),
        };
        let other = Event {
            sequence: 2,
            workspace_id: "ws".into(),
            body: Some(event::Body::IndexProgress(
                lattice_protocol::IndexProgress {
                    phase: "fts".into(),
                    path: Some("Product/Roadmap.md".into()),
                    detail: None,
                },
            )),
        };
        let filter = EventFilter {
            workspace_id: Some("ws".into()),
            agent_events_only: true,
        };
        assert!(event_matches_filter(&agent, &filter));
        assert!(!event_matches_filter(&other, &filter));
    }
}
