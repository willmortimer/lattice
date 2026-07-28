//! KernelFS MVP: scoped execution projection for WASI / agent runs.
//!
//! Materializes `/input`, `/work`, `/output`, and `/tmp` under a run directory,
//! configures Wasmtime WASI preopens, and bridges `/output` artifacts into
//! Lattice proposal overlays (see [`output_bridge`]).

mod manifest;
mod materialize;
mod output_bridge;
mod wasi_preopens;
mod wasi_runtime;

pub use manifest::{
    Capabilities, ExecutionManifest, InputMount, Mounts, NetworkPolicy, SecretHandle,
};
pub use materialize::{
    materialize, normalize_guest_path, HydrationRecord, HydrationSource, MaterializeError, RunDir,
};
pub use output_bridge::{
    collect_output_commit_plan, lattice_proposal_drafts, LatticeProposalAdapter,
    LatticeProposalDraft, OutputCommitEntry, OutputCommitPlan,
};
pub use wasi_preopens::{configure_wasi_preopens, WasiPreopenError, WasiPreopenSpec};
pub use wasi_runtime::{
    configure_engine, configure_store, engine_with_limits, WasmtimeLimits, DEFAULT_EPOCH_DEADLINE_TICKS,
    DEFAULT_FUEL_LIMIT,
};
