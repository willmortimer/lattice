//! Agent runtime backend trait.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::protocol::{
    AgentEvent, AgentRunHandle, AgentRunId, AgentRuntimeError, AgentRuntimeHealth,
    StartAgentRunRequest,
};

/// Sink for streaming [`AgentEvent`]s from a backend to the daemon / clients.
pub type AgentEventSink = mpsc::Sender<AgentEvent>;

/// Pluggable agent runtime (sidecar, fake, later cell / cloud).
#[async_trait]
pub trait AgentRuntimeBackend: Send + Sync {
    async fn start_run(
        &self,
        request: StartAgentRunRequest,
        events: AgentEventSink,
    ) -> Result<AgentRunHandle, AgentRuntimeError>;

    async fn cancel_run(&self, run_id: AgentRunId) -> Result<(), AgentRuntimeError>;

    async fn health(&self) -> Result<AgentRuntimeHealth, AgentRuntimeError>;
}
