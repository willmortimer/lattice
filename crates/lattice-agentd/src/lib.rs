//! Opt-in Rust `lattice-agentd` sidecar (ADR 0051 / 0066).
//!
//! Speaks the Phase A JSONL agent protocol over stdio. Desktop / latticed
//! auto-discover this binary; override with `LATTICE_AGENTD_BIN`.

pub mod cell_host;
pub mod fake;
pub mod kernelfs_export;
pub mod local;
pub mod lattice_client;
pub mod loop_runtime;
pub mod pioneer;
pub mod protocol;
pub mod responses;
pub mod seatbelt;
pub mod secret_handles;
pub mod tools;
pub mod wasi_host;

pub use kernelfs_export::{
    export_oci_roles_under_agent_share, OciKernelfsExport, OciKernelfsExportError,
    OciKernelfsExportRequest,
};
pub use lattice_client::{lattice_client_from_env, LatticeToolClient};
pub use local::{emit_local_run, LocalRunOptions};
pub use loop_runtime::{
    max_tool_rounds, max_tool_rounds_from_env, run_jsonl_loop, LoopConfig, DEFAULT_MAX_TOOL_ROUNDS,
    ENV_MAX_TOOL_ROUNDS, MAX_TOOL_ROUNDS_CAP, MIN_TOOL_ROUNDS,
};
pub use pioneer::{emit_pioneer_run, PioneerRunOptions};
pub use protocol::{AgentCommand, AgentEvent, ProviderKind, PROTOCOL_VERSION};
pub use responses::{emit_openai_run, OpenaiRunOptions, ResponsesError};
pub use tools::{
    emit_overlay_show_sequence, openai_tool_definitions, ToolEventSink, ToolRunContext,
    MAX_OVERLAY_ANCHORS, WORKSPACE_AGENT_INSTRUCTIONS,
};
pub use secret_handles::{parse_secret_handle_allowlist, secret_handles_from_env, SECRET_HANDLES_ENV};
pub use wasi_host::{
    hydration_inputs_from_record, propose_output_drafts, propose_output_drafts_with_provenance,
    run_wasi_guest, run_wasi_guest_with_options, unsupported_capability_error_json,
    wasi_host_error_json, wasi_materialize_error_json, wasi_run_error_json, DraftProvenance,
    HydrationInputDigest, WorkspaceBinding, WasiGuestHostOptions, WasiGuestRunResult, WasiHostError,
    WasiProposalProvenance,
};
