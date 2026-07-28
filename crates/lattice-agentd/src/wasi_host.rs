//! KernelFS WASI host helper: materialize → run `_start` → proposal drafts → latticed.
//!
//! Lattice search/read/related stay host HTTP tools. This path only bridges sandboxed
//! guest `/output` files into `propose_resource` overlays for human accept/reject.

use std::path::Path;

use kernelfs::{
    collect_output_commit_plan, configure_store, configure_wasi_preopens, engine_with_limits,
    materialize, ExecutionManifest, LatticeProposalAdapter, LatticeProposalDraft,
    MaterializeError, WasmtimeLimits, WasiPreopenError, WasiPreopenSpec,
};
use serde_json::{json, Value};
use thiserror::Error;
use wasmtime::{Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

use crate::lattice_client::{LatticeApiError, LatticeToolClient};

/// Errors from materializing a run dir and executing a WASI guest.
#[derive(Debug, Error)]
pub enum WasiHostError {
    #[error(transparent)]
    Materialize(#[from] MaterializeError),
    #[error(transparent)]
    Preopen(#[from] WasiPreopenError),
    #[error("wasmtime: {0}")]
    Wasmtime(String),
}

impl From<wasmtime::Error> for WasiHostError {
    fn from(err: wasmtime::Error) -> Self {
        Self::Wasmtime(err.to_string())
    }
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

/// Materialize KernelFS mounts, run the guest `_start` export, and collect Lattice drafts.
///
/// Sync Wasmtime/WASI entrypoint — call from a blocking thread (or `spawn_blocking`)
/// when invoked from an async runtime.
pub fn run_wasi_guest(
    run_parent: &Path,
    manifest: &ExecutionManifest,
    wasm_bytes: &[u8],
    limits: &WasmtimeLimits,
) -> Result<Vec<LatticeProposalDraft>, WasiHostError> {
    let run = materialize(run_parent, manifest)?;
    let spec = WasiPreopenSpec::from_run_root(&run.root);

    let engine = engine_with_limits(limits)?;
    let module = Module::from_binary(&engine, wasm_bytes)?;

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |ctx| ctx)?;
    let pre = linker.instantiate_pre(&module)?;

    let mut builder = WasiCtxBuilder::new();
    configure_wasi_preopens(&mut builder, &spec)?;
    let wasi = builder.build_p1();

    let mut store = Store::new(&engine, wasi);
    configure_store(&mut store, limits)?;

    let instance = pre.instantiate(&mut store)?;
    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    start.call(&mut store, ())?;

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
