//! Opt-in Rust `lattice-agentd` sidecar (ADR 0051 / 0066).
//!
//! Speaks the same Phase A JSONL protocol as Node `apps/agentd`. Desktop /
//! latticed prefer this binary by default and fall back to Node `run.sh`.

pub mod fake;
pub mod lattice_client;
pub mod loop_runtime;
pub mod pioneer;
pub mod protocol;
pub mod responses;
pub mod tools;

pub use lattice_client::{lattice_client_from_env, LatticeToolClient};
pub use loop_runtime::{run_jsonl_loop, LoopConfig};
pub use pioneer::{emit_pioneer_run, PioneerRunOptions};
pub use protocol::{AgentCommand, AgentEvent, ProviderKind, PROTOCOL_VERSION};
pub use responses::{emit_openai_run, OpenaiRunOptions, ResponsesError};
pub use tools::{openai_tool_definitions, ToolRunContext, WORKSPACE_AGENT_INSTRUCTIONS};
