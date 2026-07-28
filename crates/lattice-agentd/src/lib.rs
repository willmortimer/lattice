//! Opt-in Rust `lattice-agentd` sidecar (ADR 0051 / 0066).
//!
//! Speaks the same Phase A JSONL protocol as Node `apps/agentd`. Point
//! `LATTICE_AGENTD_BIN` at the built binary to opt in; default discovery still
//! resolves the Node tree when the env var is unset.

pub mod fake;
pub mod loop_runtime;
pub mod protocol;
pub mod responses;

pub use loop_runtime::{run_jsonl_loop, LoopConfig};
pub use protocol::{AgentCommand, AgentEvent, ProviderKind, PROTOCOL_VERSION};
