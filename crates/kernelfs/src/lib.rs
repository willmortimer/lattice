//! KernelFS: scoped execution projection for WASI / agent runs.
//!
//! Materializes `/input`, `/work`, `/output`, and `/tmp` under a run directory,
//! configures Wasmtime WASI preopens, runs guests with fuel/epoch/cancel, and
//! bridges `/output` artifacts into Lattice proposal overlays (see [`output_bridge`]).

mod manifest;
mod materialize;
mod output_bridge;
mod wasi_preopens;
mod wasi_run;
mod wasi_runtime;

pub use manifest::{
    Capabilities, ExecutionManifest, InputMount, Mounts, NetworkPolicy, SecretHandle,
};
pub use materialize::{
    materialize, materialize_with_options, normalize_guest_path, HydrationRecord, HydrationSource,
    MaterializeError, MaterializeOptions, RunDir,
};
pub use output_bridge::{
    collect_output_commit_plan, lattice_proposal_drafts, LatticeProposalAdapter,
    LatticeProposalDraft, OutputCommitEntry, OutputCommitPlan,
};
pub use wasi_preopens::{configure_wasi_preopens, WasiPreopenError, WasiPreopenSpec};
pub use wasi_run::{
    run_wasi_guest, WasiRunError, WasiRunOptions, WasiRunResult, DEFAULT_EPOCH_TICK_INTERVAL,
    DEFAULT_STDIO_CAPTURE_CAPACITY,
};
pub use wasi_runtime::{
    configure_engine, configure_store, engine_with_limits, WasmtimeLimits,
    DEFAULT_EPOCH_DEADLINE_TICKS, DEFAULT_FUEL_LIMIT,
};
