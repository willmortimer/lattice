//! KernelFS WASI host helper: materialize → run `_start` → proposal drafts → latticed.
//!
//! Lattice search/read/related stay host HTTP tools. This path only bridges sandboxed
//! guest `/output` files into `propose_resource` overlays for human accept/reject.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use kernelfs::{
    collect_output_commit_plan, materialize_with_options, run_wasi_guest as kernelfs_run,
    ContentKind, ExecutionManifest, HostPathPolicy, HydrationRecord, LatticeProposalAdapter,
    LatticeProposalDraft, MaterializeError, MaterializeOptions, SecretHandleEntry,
    SecretHandlePolicy, WasmtimeLimits, WasiRunError, WasiRunOptions, WasiRunResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use base64::Engine;

use crate::lattice_client::{LatticeApiError, LatticeToolClient};
use crate::seatbelt::{self, SeatbeltError};

/// Max characters of stdout/stderr tails embedded in structured tool errors.
pub const WASI_STDIO_TAIL_CHARS: usize = 2_000;

/// Errors from materializing a run dir and executing a WASI guest.
#[derive(Debug, Error)]
pub enum WasiHostError {
    #[error(transparent)]
    Materialize(#[from] MaterializeError),
    #[error(transparent)]
    Run(#[from] WasiRunError),
    #[error(transparent)]
    Seatbelt(#[from] SeatbeltError),
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
    /// Host paths allowed for manifest secret handles (`/run/secrets/<id>`).
    pub secret_handle_allowlist: Vec<SecretHandleEntry>,
}

/// Provenance attached to proposed KernelFS output drafts.
pub trait DraftProvenance {
    fn source_resource(&self) -> String;
    fn enrich_summary(&self, base: &str) -> String;
    /// Structured hydration digests persisted on the propose body / proposal source.
    fn hydration_inputs(&self) -> &[HydrationInputDigest] {
        &[]
    }
}

/// One hydration input digest for LatticeFS accept lineage (mirrors lattice-commands).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HydrationInputDigest {
    pub path: String,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
}

/// Build digests from a KernelFS [`HydrationRecord`], attaching optional ResourceIds by guest path.
pub fn hydration_inputs_from_record(
    record: &HydrationRecord,
    resource_ids: &BTreeMap<String, String>,
) -> Vec<HydrationInputDigest> {
    record
        .sources
        .iter()
        .map(|source| HydrationInputDigest {
            path: source.guest_path.clone(),
            content_hash: source.sha256.clone(),
            resource_id: resource_ids.get(&source.guest_path).cloned(),
        })
        .collect()
}

/// Provenance attached to proposed WASI `/output` drafts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiProposalProvenance {
    pub run_id: String,
    pub wasm_path: String,
    pub output_proposal_target: String,
    /// Guest path + content hash (+ optional ResourceId) from KernelFS hydration.
    pub hydration_inputs: Vec<HydrationInputDigest>,
}

impl DraftProvenance for WasiProposalProvenance {
    fn source_resource(&self) -> String {
        format!("wasi://{}/{}", self.run_id, self.wasm_path.trim_start_matches('/'))
    }

    fn enrich_summary(&self, base: &str) -> String {
        let inputs = self
            .hydration_inputs
            .iter()
            .map(|digest| format!("{}@{}", digest.path, digest.content_hash))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{base} [wasi runId={} wasm={} target={} inputs=[{}]]",
            self.run_id, self.wasm_path, self.output_proposal_target, inputs
        )
    }

    fn hydration_inputs(&self) -> &[HydrationInputDigest] {
        &self.hydration_inputs
    }
}

/// Result of a successful KernelFS WASI host run.
#[derive(Debug, Clone)]
pub struct WasiGuestRunResult {
    pub drafts: Vec<LatticeProposalDraft>,
    pub hydration: HydrationRecord,
    pub run: WasiRunResult,
}

/// Map [`WasiRunError`] into structured tool JSON (`kind` + stdio tails).
pub fn wasi_run_error_json(err: &WasiRunError) -> Value {
    match err {
        WasiRunError::FuelExhausted { stdout, stderr } => json!({
            "kind": "fuel_exhausted",
            "message": err.to_string(),
            "stdoutTail": stdio_tail(stdout),
            "stderrTail": stdio_tail(stderr),
        }),
        WasiRunError::EpochDeadline { stdout, stderr } => json!({
            "kind": "epoch_deadline",
            "message": err.to_string(),
            "stdoutTail": stdio_tail(stdout),
            "stderrTail": stdio_tail(stderr),
        }),
        WasiRunError::Cancelled { stdout, stderr } => json!({
            "kind": "cancelled",
            "message": err.to_string(),
            "stdoutTail": stdio_tail(stdout),
            "stderrTail": stdio_tail(stderr),
        }),
        WasiRunError::MissingStart => json!({
            "kind": "missing_start",
            "message": err.to_string(),
            "stdoutTail": "",
            "stderrTail": "",
        }),
        WasiRunError::Trap {
            message,
            stdout,
            stderr,
        } => json!({
            "kind": "trap",
            "message": message,
            "stdoutTail": stdio_tail(stdout),
            "stderrTail": stdio_tail(stderr),
        }),
        WasiRunError::Engine(inner) => json!({
            "kind": "engine",
            "message": inner.to_string(),
            "stdoutTail": "",
            "stderrTail": "",
        }),
        WasiRunError::Preopen(inner) => json!({
            "kind": "preopen",
            "message": inner.to_string(),
            "stdoutTail": "",
            "stderrTail": "",
        }),
    }
}

fn stdio_tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.chars().count() <= WASI_STDIO_TAIL_CHARS {
        return text.into_owned();
    }
    let truncated: String = text
        .chars()
        .rev()
        .take(WASI_STDIO_TAIL_CHARS)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("…{truncated}")
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
) -> Result<WasiGuestRunResult, WasiHostError> {
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
) -> Result<WasiGuestRunResult, WasiHostError> {
    let secret_handle_policy = if options.secret_handle_allowlist.is_empty() {
        SecretHandlePolicy::DenyAll
    } else {
        SecretHandlePolicy::AllowHandles(&options.secret_handle_allowlist)
    };
    let run_dir = materialize_with_options(
        run_parent,
        manifest,
        &MaterializeOptions {
            host_path_policy: HostPathPolicy::AllowRoots(&options.host_path_roots),
            secret_handle_policy,
        },
    )?;

    let mut run_opts = WasiRunOptions {
        limits: options.limits.clone(),
        cancel: options.cancel.clone(),
        ..WasiRunOptions::default()
    };
    if options.max_wall_time.is_some() {
        run_opts.max_wall_time = options.max_wall_time;
    }

    let run = if seatbelt::seatbelt_enabled() {
        match seatbelt::run_wasi_in_seatbelt(&run_dir.root, wasm_bytes, &run_opts) {
            Ok(result) => result,
            Err(SeatbeltError::Guest(err)) => return Err(WasiHostError::Run(err)),
            Err(SeatbeltError::Cancelled) => {
                return Err(WasiHostError::Run(WasiRunError::Cancelled {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }));
            }
            Err(SeatbeltError::RunnerMissing) => {
                // Incomplete install / unit tests without the helper: keep running
                // in-process so Linux CI and local debug still work, but warn.
                tracing::warn!(
                    target: "lattice_agentd",
                    "Seatbelt enabled but lattice-wasi-seatbelt missing; falling back to in-process Wasmtime"
                );
                kernelfs_run(&run_dir.root, wasm_bytes, &run_opts)?
            }
            Err(SeatbeltError::UnsupportedPlatform) => {
                return Err(WasiHostError::Seatbelt(SeatbeltError::UnsupportedPlatform));
            }
            Err(err) => return Err(WasiHostError::Seatbelt(err)),
        }
    } else {
        kernelfs_run(&run_dir.root, wasm_bytes, &run_opts)?
    };

    let plan = collect_output_commit_plan(&run_dir.root, manifest)?;
    let adapter = LatticeProposalAdapter::from_manifest(manifest);
    Ok(WasiGuestRunResult {
        drafts: adapter.drafts(&plan),
        hydration: run_dir.hydration,
        run,
    })
}

/// Push each draft via `POST /v1/proposals/propose_resource` (Node/latticed body shape).
pub async fn propose_output_drafts(
    client: &LatticeToolClient,
    workspace: &WorkspaceBinding,
    drafts: &[LatticeProposalDraft],
) -> Result<Vec<Value>, ProposeDraftsError> {
    propose_output_drafts_with_provenance::<WasiProposalProvenance>(client, workspace, drafts, None)
        .await
}

/// Like [`propose_output_drafts`], with optional provenance on each body.
pub async fn propose_output_drafts_with_provenance<P: DraftProvenance + Sync>(
    client: &LatticeToolClient,
    workspace: &WorkspaceBinding,
    drafts: &[LatticeProposalDraft],
    provenance: Option<&P>,
) -> Result<Vec<Value>, ProposeDraftsError> {
    if !workspace.is_bound() {
        return Err(ProposeDraftsError::MissingWorkspace);
    }

    let mut results = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let summary = match provenance {
            Some(prov) => prov.enrich_summary(&draft.summary),
            None => draft.summary.clone(),
        };

        let mut body = match draft.kind {
            ContentKind::Text => {
                let content = std::str::from_utf8(&draft.content).map_err(|_| {
                    ProposeDraftsError::NonUtf8Content {
                        path: draft.resource_path.clone(),
                    }
                })?;
                json!({
                    "path": draft.resource_path,
                    "content": content,
                    "summary": summary,
                })
            }
            ContentKind::Bytes => json!({
                "path": draft.resource_path,
                "contentBase64": base64::engine::general_purpose::STANDARD.encode(&draft.content),
                "summary": summary,
            }),
        };
        if let Some(prov) = provenance {
            body["sourceResource"] = Value::String(prov.source_resource());
            let digests = prov.hydration_inputs();
            if !digests.is_empty() {
                body["hydrationInputs"] =
                    serde_json::to_value(digests).unwrap_or_else(|_| Value::Array(Vec::new()));
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use kernelfs::{Capabilities, SecretHandle};

    #[test]
    fn deny_all_when_secret_allowlist_empty() {
        let temp = tempfile::tempdir().expect("tempdir");
        let secret = temp.path().join("api-key.txt");
        std::fs::write(&secret, b"secret").expect("write secret");
        let manifest = ExecutionManifest {
            run_id: "run_secret_deny".into(),
            base_snapshot: "snap".into(),
            mounts: Default::default(),
            capabilities: Capabilities {
                secrets: vec![SecretHandle {
                    id: "api-key".into(),
                }],
                ..Default::default()
            },
        };
        let err = materialize_with_options(
            temp.path(),
            &manifest,
            &MaterializeOptions {
                host_path_policy: HostPathPolicy::UnrestrictedForTests,
                secret_handle_policy: SecretHandlePolicy::DenyAll,
            },
        )
        .expect_err("deny by default");
        assert!(matches!(
            err,
            MaterializeError::SecretHandleNotAllowed { .. }
        ));
    }

    #[test]
    fn maps_fuel_exhausted_to_structured_json() {
        let err = WasiRunError::FuelExhausted {
            stdout: b"out".to_vec(),
            stderr: b"err".to_vec(),
        };
        let value = wasi_run_error_json(&err);
        assert_eq!(value["kind"], "fuel_exhausted");
        assert_eq!(value["stdoutTail"], "out");
        assert_eq!(value["stderrTail"], "err");
    }

    #[test]
    fn provenance_source_resource_and_summary() {
        let prov = WasiProposalProvenance {
            run_id: "run_1".into(),
            wasm_path: "Tools/guests/copy_hello.wasm".into(),
            output_proposal_target: "Reports".into(),
            hydration_inputs: vec![HydrationInputDigest {
                path: "hello.txt".into(),
                content_hash: "abc".into(),
                resource_id: Some("rid-1".into()),
            }],
        };
        assert_eq!(
            prov.source_resource(),
            "wasi://run_1/Tools/guests/copy_hello.wasm"
        );
        let summary = prov.enrich_summary("Create resource Reports/out.txt");
        assert!(summary.contains("runId=run_1"));
        assert!(summary.contains("hello.txt@abc"));
        assert!(summary.contains("target=Reports"));
        assert_eq!(prov.hydration_inputs()[0].resource_id.as_deref(), Some("rid-1"));
    }

    #[test]
    fn hydration_inputs_from_record_attaches_optional_resource_id() {
        let record = HydrationRecord {
            run_id: "run_x".into(),
            base_snapshot: "snap".into(),
            root: std::path::PathBuf::from("/tmp/run_x"),
            sources: vec![kernelfs::HydrationSource {
                guest_path: "hello.txt".into(),
                host_path: std::path::PathBuf::from("/tmp/hello.txt"),
                sha256: "deadbeef".into(),
            }],
        };
        let mut ids = BTreeMap::new();
        ids.insert("hello.txt".into(), "res-42".into());
        let digests = hydration_inputs_from_record(&record, &ids);
        assert_eq!(digests.len(), 1);
        assert_eq!(digests[0].path, "hello.txt");
        assert_eq!(digests[0].content_hash, "deadbeef");
        assert_eq!(digests[0].resource_id.as_deref(), Some("res-42"));
    }
}
