//! Embedded agent supervision for `latticed` (Phase A).
//!
//! `latticed` owns the agent process lifecycle. Desktop / Tauri must not spawn
//! `agentd` directly — see `docs/architecture/embedded-agent.md` and ADR 0044.

mod backend;
mod controller;
mod fake;
mod protocol;
mod sidecar;

pub use backend::{AgentEventSink, AgentRuntimeBackend};
pub use controller::{AgentController, AgentProviderMode};
pub use fake::FakeAgentBackend;
pub use protocol::{
    AgentCommand, AgentEvent, AgentRunHandle, AgentRunId, AgentRuntimeError, AgentRuntimeHealth,
    ProviderKind, StartAgentRunRequest, PROTOCOL_VERSION,
};
pub use sidecar::{
    resolve_agentd_bin, SidecarAgentBackend, ENV_AGENTD_BIN, ENV_AGENT_FAKE, ENV_AGENT_MODEL,
    ENV_AGENT_PROVIDER,
};
