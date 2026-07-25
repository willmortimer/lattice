//! Daemon-facing agent controller (wraps Fake / Sidecar backends).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lattice_protocol::{event, Event};
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

use super::backend::AgentRuntimeBackend;
use super::fake::FakeAgentBackend;
use super::protocol::{
    AgentEvent, AgentRunHandle, AgentRunId, AgentRuntimeError, AgentRuntimeHealth, ProviderKind,
    StartAgentRunRequest,
};
use super::sidecar::{
    default_model_from_env, default_provider_from_env, env_truthy, resolve_agentd_bin,
    SidecarAgentBackend, ENV_AGENT_FAKE,
};

/// How the daemon obtains an agent runtime.
#[derive(Debug, Clone)]
pub enum AgentProviderMode {
    /// In-process deterministic backend (tests / CI).
    Fake,
    /// Spawn and supervise `agentd` over JSONL stdio.
    Sidecar {
        binary: std::path::PathBuf,
        args: Vec<String>,
    },
}

impl AgentProviderMode {
    /// Resolve from environment.
    ///
    /// - `LATTICE_AGENT_FAKE=1` → Fake (even if `LATTICE_AGENTD_BIN` is set)
    /// - `LATTICE_AGENTD_BIN` → Sidecar
    /// - otherwise → `None` (agent plane disabled)
    pub fn from_env() -> Option<Self> {
        if env_truthy(ENV_AGENT_FAKE) {
            return Some(Self::Fake);
        }
        resolve_agentd_bin().map(|(binary, args)| Self::Sidecar { binary, args })
    }
}

/// Shared agent controller for a daemon instance.
pub struct AgentController {
    backend: Arc<dyn AgentRuntimeBackend>,
    backend_name: String,
    sidecar: Option<Arc<SidecarAgentBackend>>,
    event_fanout: Mutex<Option<(broadcast::Sender<Event>, Arc<AtomicU64>)>>,
}

impl AgentController {
    /// Build and start a controller for the given mode.
    pub async fn start(mode: AgentProviderMode) -> Result<Arc<Self>, AgentRuntimeError> {
        match mode {
            AgentProviderMode::Fake => {
                info!("agent controller using FakeAgentBackend");
                Ok(Arc::new(Self {
                    backend: Arc::new(FakeAgentBackend::new()),
                    backend_name: "fake".into(),
                    sidecar: None,
                    event_fanout: Mutex::new(None),
                }))
            }
            AgentProviderMode::Sidecar { binary, args } => {
                info!(binary = %binary.display(), "agent controller using SidecarAgentBackend");
                let sidecar = SidecarAgentBackend::new(binary, args);
                sidecar.start().await?;
                let backend = Arc::clone(&sidecar) as Arc<dyn AgentRuntimeBackend>;
                Ok(Arc::new(Self {
                    backend,
                    backend_name: "sidecar".into(),
                    sidecar: Some(sidecar),
                    event_fanout: Mutex::new(None),
                }))
            }
        }
    }

    /// Attach sequenced event fan-out onto the daemon client event bus.
    pub fn attach_event_fanout(
        &self,
        event_tx: broadcast::Sender<Event>,
        next_event_seq: Arc<AtomicU64>,
    ) {
        *self.event_fanout.lock().expect("event_fanout poisoned") =
            Some((event_tx, next_event_seq));
    }

    pub fn backend_name(&self) -> &str {
        &self.backend_name
    }

    pub fn is_degraded(&self) -> bool {
        self.sidecar
            .as_ref()
            .map(|s| s.is_degraded())
            .unwrap_or(false)
    }

    /// Start an agent run; events are pushed to the optional daemon event bus.
    pub async fn start_run(
        &self,
        mut request: StartAgentRunRequest,
    ) -> Result<AgentRunHandle, AgentRuntimeError> {
        if request.run_id.as_str().is_empty() {
            request.run_id = AgentRunId::new(Uuid::now_v7().to_string());
        }
        if request.provider == ProviderKind::Fake && request.model.is_empty() {
            request.model = default_model_from_env();
        }
        if request.model.is_empty() {
            request.model = default_model_from_env();
        }

        let workspace_id = request.workspace_id.clone();
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(1024);
        let handle = self.backend.start_run(request, tx).await?;

        if let Some((event_tx, next_seq)) = self
            .event_fanout
            .lock()
            .expect("event_fanout poisoned")
            .clone()
        {
            let run_id = handle.run_id.as_str().to_string();
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let payload_json = match serde_json::to_string(&event) {
                        Ok(json) => json,
                        Err(err) => {
                            warn!(error = %err, "failed to serialize agent event");
                            continue;
                        }
                    };
                    let sequenced = Event {
                        sequence: next_seq.fetch_add(1, Ordering::Relaxed),
                        workspace_id: workspace_id.clone(),
                        body: Some(event::Body::AgentEvent(lattice_protocol::AgentEvent {
                            run_id: event.run_id().unwrap_or(&run_id).to_string(),
                            event_type: event.event_type().to_string(),
                            payload_json,
                        })),
                    };
                    let _ = event_tx.send(sequenced);
                }
            });
        } else {
            // No fan-out attached (unit tests): drain so the sender does not block.
            tokio::spawn(async move { while rx.recv().await.is_some() {} });
        }

        Ok(handle)
    }

    pub async fn cancel_run(&self, run_id: AgentRunId) -> Result<(), AgentRuntimeError> {
        self.backend.cancel_run(run_id).await
    }

    pub async fn health(&self) -> Result<AgentRuntimeHealth, AgentRuntimeError> {
        let mut health = self.backend.health().await?;
        // Badge / health UI expect a provider kind (`fake` | `pioneer` | `openai`),
        // not the transport name (`sidecar`).
        health.backend = match self.backend_name.as_str() {
            "fake" => "fake".into(),
            _ => self.default_provider().as_str().to_string(),
        };
        health.degraded = health.degraded || self.is_degraded();
        health.ok = health.ok && !health.degraded;
        Ok(health)
    }

    /// Defaults for RPC callers that omit provider/model.
    pub fn default_provider(&self) -> ProviderKind {
        if self.backend_name == "fake" {
            ProviderKind::Fake
        } else {
            default_provider_from_env()
        }
    }

    pub fn default_model(&self) -> String {
        default_model_from_env()
    }

    pub async fn shutdown(&self) {
        if let Some(sidecar) = &self.sidecar {
            sidecar.shutdown().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn controller_fake_start_cancel_health() {
        let controller = AgentController::start(AgentProviderMode::Fake)
            .await
            .expect("start");
        let health = controller.health().await.expect("health");
        assert!(health.ok);
        assert_eq!(health.backend, "fake");

        let handle = controller
            .start_run(StartAgentRunRequest {
                thread_id: "t".into(),
                run_id: AgentRunId::new("run-ctrl"),
                provider: ProviderKind::Fake,
                model: "fake".into(),
                messages: None,
                prompt: Some("hello".into()),
                workspace_id: "ws".into(),
                workspace_root: None,
            })
            .await
            .expect("start_run");
        assert_eq!(handle.run_id.as_str(), "run-ctrl");

        // Cancel may race with natural completion; either outcome is fine.
        let _ = controller.cancel_run(AgentRunId::new("run-ctrl")).await;
    }
}
