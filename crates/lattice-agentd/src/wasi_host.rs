//! KernelFS WASI host helper: materialize → run `_start` → proposal drafts → latticed.
//!
//! Lattice search/read/related stay host HTTP tools. This path only bridges sandboxed
//! guest `/output` files into `propose_resource` overlays for human accept/reject.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use kernelfs::{
    collect_output_commit_plan, materialize, materialize_with_options, run_wasi_guest as kernelfs_run,
    ExecutionManifest, LatticeProposalAdapter, LatticeProposalDraft, MaterializeError,
    MaterializeOptions, WasmtimeLimits, WasiRunError, WasiRunOptions,
};
use serde_json::{json, Value};
use thiserror::Error;

use crate::lattice_client::{LatticeApiError, LatticeToolClient};

/// Errors from materializing a run dir and executing a WASI guest.
#[derive(Debug, Error)]
pub enum WasiHostError {
    #[error(transparent)]
    Materialize(#[from] MaterializeError),
    #[error(transparent)]
    Run(#[from] WasiRunError),
}

/// Workspace binding for latticed proposal routes (`workspaceId` and/or `root`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBinding {
    pub workspace_id: Option<String>,
    pub root: Option<String>,
}

impl WorkspaceBinding {
    pub fn new(workspace_id: Option<String>, root: Option<String>) -> Self {
        Self {
            workspace_id,
            root,
        }
    }

    pub fn is_bound(&self) -> bool {
        self.workspace_id
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
            || self.root.as_ref().is_some_and(|s| !s.trim().is_empty())
    }
}

/// Errors pushing KernelFS proposal drafts through latticed.
#[derive(Debug, Error)]
pub enum ProposeDraftsError {
    #[error(
        "workspace binding required: pass workspaceId or root before proposing KernelFS output"
    )]
    MissingWorkspace,
    #[error("proposal draft content is not valid UTF-8 at {path}")]
    NonUtf8Content { path: String },
    #[error(transparent)]
    Api(#[from] LatticeApiError),
}

/// Options for [`run_wasi_guest`] beyond the KernelFS defaults.
#[derive(Debug, Clone, Default)]
pub struct WasiGuestHostOptions {
    pub limits: WasmtimeLimits,
    pub max_wall_time: Option<Duration>,
    pub cancel: Option<Arc<AtomicBool>>,
    /// When set, input host paths must canonicalize under these roots.
    pub host_path_roots: Vec<std::path::PathBuf>,
}

/// Materialize KernelFS mounts, run the guest `_start` export, and collect Lattice drafts.
///
/// Uses [`kernelfs::run_wasi_guest`] (epoch ticker + cancel + stdio). Call from
/// `spawn_blocking` when invoked from an async runtime.
pub fn run_wasi_guest(
    run_parent: &Path,
    manifest: &ExecutionManifest,
    wasm_bytes: &[u8],
    limits: &WasmtimeLimits,
) -> Result<Vec<LatticeProposalDraft>, WasiHostError> {
    run_wasi_guest_with_options(
        run_parent,
        manifest,
        wasm_bytes,
        &WasiGuestHostOptions {
            limits: limits.clone(),
            ..Default::default()
        },
    )
}

/// Like [`run_wasi_guest`], with cancel / wall-time / host-path allowlist.
pub fn run_wasi_guest_with_options(
    run_parent: &Path,
    manifest: &ExecutionManifest,
    wasm_bytes: &[u8],
    options: &WasiGuestHostOptions,
) -> Result<Vec<LatticeProposalDraft>, WasiHostError> {
    let run = if options.host_path_roots.is_empty() {
        materialize(run_parent, manifest)?
    } else {
        materialize_with_options(
            run_parent,
            manifest,
            &MaterializeOptions {
                host_path_roots: &options.host_path_roots,
            },
        )?
    };

    let mut run_opts = WasiRunOptions {
        limits: options.limits.clone(),
        cancel: options.cancel.clone(),
        ..WasiRunOptions::default()
    };
    if options.max_wall_time.is_some() {
        run_opts.max_wall_time = options.max_wall_time;
    }

    let _result = kernelfs_run(&run.root, wasm_bytes, &run_opts)?;

    let plan = collect_output_commit_plan(&run.root, manifest)?;
    let adapter = LatticeProposalAdapter::from_manifest(manifest);
    Ok(adapter.drafts(&plan))
}

/// Push each draft via `POST /v1/proposals/propose_resource` (Node/latticed body shape).
pub async fn propose_output_drafts(
    client: &LatticeToolClient,
    workspace: &WorkspaceBinding,
    drafts: &[LatticeProposalDraft],
) -> Result<Vec<Value>, ProposeDraftsError> {
    if !workspace.is_bound() {
        return Err(ProposeDraftsError::MissingWorkspace);
    }

    let mut results = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let content = std::str::from_utf8(&draft.content).map_err(|_| {
            ProposeDraftsError::NonUtf8Content {
                path: draft.resource_path.clone(),
            }
        })?;

        let mut body = json!({
            "path": draft.resource_path,
            "content": content,
            "summary": draft.summary,
        });
        if let Some(id) = workspace
            .workspace_id
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            body["workspaceId"] = Value::String(id.clone());
        }
        if let Some(root) = workspace.root.as_ref().filter(|s| !s.trim().is_empty()) {
            body["root"] = Value::String(root.clone());
        }

        results.push(client.propose_resource(body).await?);
    }
    Ok(results)
}
