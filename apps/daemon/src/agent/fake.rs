//! In-process deterministic agent backend (no Node / agentd).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::debug;

use super::backend::{AgentEventSink, AgentRuntimeBackend};
use super::protocol::{
    AgentEvent, AgentRunHandle, AgentRunId, AgentRuntimeError, AgentRuntimeHealth, ProviderKind,
    StartAgentRunRequest,
};

struct ActiveFakeRun {
    cancel: Arc<AtomicBool>,
}

/// Emits a short synthetic stream for daemon tests without spawning Node.
pub struct FakeAgentBackend {
    active: Arc<Mutex<HashMap<String, ActiveFakeRun>>>,
}

impl FakeAgentBackend {
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for FakeAgentBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentRuntimeBackend for FakeAgentBackend {
    async fn start_run(
        &self,
        request: StartAgentRunRequest,
        events: AgentEventSink,
    ) -> Result<AgentRunHandle, AgentRuntimeError> {
        request.validate()?;
        let run_id = request.run_id.clone();
        let thread_id = request.thread_id.clone();
        let cancel = Arc::new(AtomicBool::new(false));

        {
            let mut active = self.active.lock().await;
            if active.contains_key(run_id.as_str()) {
                return Err(AgentRuntimeError::InvalidRequest(format!(
                    "run already active: {run_id}"
                )));
            }
            active.insert(
                run_id.as_str().to_string(),
                ActiveFakeRun {
                    cancel: Arc::clone(&cancel),
                },
            );
        }

        let run_id_task = run_id.clone();
        let active_map = Arc::clone(&self.active);
        tokio::spawn(async move {
            let send = |event: AgentEvent| {
                let tx = events.clone();
                async move {
                    let _ = tx.send(event).await;
                }
            };

            send(AgentEvent::RunStarted {
                run_id: run_id_task.0.clone(),
                thread_id: thread_id.clone(),
                provider: Some(ProviderKind::Fake),
            })
            .await;

            let chunks = ["Hello", " from ", "FakeAgentBackend."];
            for (idx, piece) in chunks.iter().enumerate() {
                if cancel.load(Ordering::SeqCst) {
                    send(AgentEvent::RunFailed {
                        run_id: run_id_task.0.clone(),
                        message: "cancelled".into(),
                        retryable: false,
                    })
                    .await;
                    active_map.lock().await.remove(run_id_task.as_str());
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
                send(AgentEvent::MessageChunk {
                    run_id: run_id_task.0.clone(),
                    chunk: serde_json::json!({
                        "type": "text-delta",
                        "id": format!("fake-{}", idx),
                        "delta": piece,
                    }),
                })
                .await;
            }

            if cancel.load(Ordering::SeqCst) {
                send(AgentEvent::RunFailed {
                    run_id: run_id_task.0.clone(),
                    message: "cancelled".into(),
                    retryable: false,
                })
                .await;
            } else {
                send(AgentEvent::RunCompleted {
                    run_id: run_id_task.0.clone(),
                })
                .await;
            }
            active_map.lock().await.remove(run_id_task.as_str());
        });

        debug!(run_id = %run_id, "fake agent run started");
        Ok(AgentRunHandle {
            run_id,
            thread_id: request.thread_id,
        })
    }

    async fn cancel_run(&self, run_id: AgentRunId) -> Result<(), AgentRuntimeError> {
        let active = self.active.lock().await;
        let Some(run) = active.get(run_id.as_str()) else {
            return Err(AgentRuntimeError::RunNotFound(run_id.to_string()));
        };
        run.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn health(&self) -> Result<AgentRuntimeHealth, AgentRuntimeError> {
        Ok(AgentRuntimeHealth {
            ok: true,
            backend: "fake".into(),
            degraded: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::protocol::{AgentRunId, ProviderKind};
    use std::time::Duration;
    use tokio::sync::mpsc;

    /// Drain a bounded event channel until a terminal run event or timeout.
    async fn collect_events_until_terminal(
        rx: &mut mpsc::Receiver<AgentEvent>,
        limit: usize,
    ) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        while out.len() < limit {
            match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                Ok(Some(event)) => {
                    let terminal = matches!(
                        event,
                        AgentEvent::RunCompleted { .. } | AgentEvent::RunFailed { .. }
                    );
                    out.push(event);
                    if terminal {
                        break;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
        out
    }

    #[tokio::test]
    async fn fake_lifecycle_start_health_and_complete() {
        let backend = FakeAgentBackend::new();
        let health = backend.health().await.expect("health");
        assert!(health.ok);
        assert_eq!(health.backend, "fake");

        let (tx, mut rx) = mpsc::channel(16);
        let handle = backend
            .start_run(
                StartAgentRunRequest {
                    thread_id: "thread-1".into(),
                    run_id: AgentRunId::new("run-1"),
                    provider: ProviderKind::Fake,
                    model: "fake-model".into(),
                    messages: None,
                    prompt: Some("hi".into()),
                    workspace_id: "ws-1".into(),
                    workspace_root: None,
                },
                tx,
            )
            .await
            .expect("start");
        assert_eq!(handle.run_id.as_str(), "run-1");

        let events = collect_events_until_terminal(&mut rx, 16).await;
        assert!(matches!(
            events.first(),
            Some(AgentEvent::RunStarted { .. })
        ));
        assert!(events
            .iter()
            .any(|e| matches!(e, AgentEvent::MessageChunk { .. })));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::RunCompleted { run_id }) if run_id == "run-1"
        ));
    }

    #[tokio::test]
    async fn fake_cancel_marks_run_failed() {
        let backend = FakeAgentBackend::new();
        let (tx, mut rx) = mpsc::channel(16);
        backend
            .start_run(
                StartAgentRunRequest {
                    thread_id: "thread-2".into(),
                    run_id: AgentRunId::new("run-2"),
                    provider: ProviderKind::Fake,
                    model: "fake-model".into(),
                    messages: None,
                    prompt: Some("cancel me".into()),
                    workspace_id: "ws-1".into(),
                    workspace_root: None,
                },
                tx,
            )
            .await
            .expect("start");

        backend
            .cancel_run(AgentRunId::new("run-2"))
            .await
            .expect("cancel");

        let events = collect_events_until_terminal(&mut rx, 16).await;
        assert!(
            events.iter().any(|e| {
                matches!(
                    e,
                    AgentEvent::RunFailed { message, .. } if message == "cancelled"
                )
            }) || matches!(events.last(), Some(AgentEvent::RunCompleted { .. })),
            "expected cancel or early completion, got {events:?}"
        );
    }
}
