//! Lattice → `celld` Connect/HTTP client for KernelFS guest hydrate/run/collect.
//!
//! Prefer the guest mirror path (`lattice.runtime.v1` / `cell.mirror.v1`) over
//! live VirtioFS binds. See `docs/dev/celld-client.md`.

mod client;
mod config;
pub mod connect;
mod error;
mod hydrate;
mod types;

pub use client::{
    celld_configured, default_client, CelldClient, CelldHttpClient, HttpCelldClient, OutputFile,
    OutputFileMap, ProjectionRunRequest, ProjectionRunResult, CELL_MIRROR_V1,
    DEFAULT_ADVERTISE_SERVICES, LATTICE_RUNTIME_V1,
};
pub use config::{CELLD_BASE_URL_ENV, celld_base_url, require_celld_base_url};
pub use error::{CellClientError, Result};
pub use hydrate::{
    AttachmentMode, HostGuestPath, KernelFSHydrationPlan, KernelFSRole, NetworkAttachment,
    VolumeAttachment, cell_spec_network_attachments, cell_spec_volume_attachments,
    hydrate_files_under_role, is_oci_execution_mode, oci_suppresses_network_deny_all,
    DEFAULT_INPUT_MOUNT, DEFAULT_OUTPUT_MOUNT, DEFAULT_WORK_MOUNT, EXECUTION_MODE_OCI,
    ROLE_INPUT, ROLE_OUTPUT, ROLE_WORK,
};
pub use types::{
    ApplyCellRequest, ApplyCellResponse, Cell, CellSpec, CollectOutputRequest,
    CollectOutputResponse, HydrateFile, HydrateProjectionRequest, HydrateProjectionResponse,
    Operation, ProfileRef, ResourceSpec, RunTaskRequest, RunTaskResponse, StartCellRequest,
    StartCellResponse,
};
