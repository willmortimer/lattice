//! celld guest hydrate → run → collect → Lattice proposal drafts (ADR 0063).
//!
//! Maps [`lattice_cell_client::OutputFileMap`] into KernelFS [`LatticeProposalDraft`]
//! values and pushes them through the same `propose_resource` path as WASI guests.

use std::collections::BTreeMap;

use kernelfs::{classify_content, LatticeProposalDraft};
use lattice_cell_client::{
    CelldClient, CelldHttpClient, OutputFileMap, ProjectionRunRequest, ProjectionRunResult,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use thiserror::Error;

use crate::lattice_client::LatticeToolClient;
use crate::wasi_host::{
    propose_output_drafts_with_provenance, DraftProvenance, HydrationInputDigest, ProposeDraftsError,
    WorkspaceBinding,
};

/// Errors from celld run + proposal bridging.
#[derive(Debug, Error)]
pub enum CellHostError {
    #[error(transparent)]
    Cell(#[from] lattice_cell_client::CellClientError),
    #[error(transparent)]
    Propose(#[from] ProposeDraftsError),
}

/// Provenance attached to proposed celld `/output` drafts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellProposalProvenance {
    pub cell_id: String,
    pub projection_id: String,
    pub task_id: String,
    pub output_proposal_target: String,
    /// Hydration digests for mirror inputs (path + content hash; optional ResourceId).
    pub hydration_inputs: Vec<HydrationInputDigest>,
}

impl DraftProvenance for CellProposalProvenance {
    fn source_resource(&self) -> String {
        format!("cell://{}/{}", self.cell_id, self.projection_id)
    }

    fn enrich_summary(&self, base: &str) -> String {
        let inputs = self
            .hydration_inputs
            .iter()
            .map(|digest| format!("{}@{}", digest.path, digest.content_hash))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{base} [cell cellId={} projection={} task={} target={} inputs=[{}]]",
            self.cell_id, self.projection_id, self.task_id, self.output_proposal_target, inputs
        )
    }

    fn hydration_inputs(&self) -> &[HydrationInputDigest] {
        &self.hydration_inputs
    }
}

/// Hash hydrate-file payloads into proposal provenance digests.
pub fn hydration_inputs_from_files(
    files: &[lattice_cell_client::HydrateFile],
    resource_ids: &BTreeMap<String, String>,
) -> Vec<HydrationInputDigest> {
    files
        .iter()
        .filter_map(|file| {
            let bytes = hydrate_file_bytes(file)?;
            let path = file
                .path
                .strip_prefix("input/")
                .unwrap_or(file.path.as_str())
                .trim_start_matches('/')
                .to_string();
            let content_hash = hex::encode(Sha256::digest(&bytes));
            Some(HydrationInputDigest {
                resource_id: resource_ids
                    .get(&path)
                    .cloned()
                    .or_else(|| resource_ids.get(&file.path).cloned()),
                path,
                content_hash,
            })
        })
        .collect()
}

fn hydrate_file_bytes(file: &lattice_cell_client::HydrateFile) -> Option<Vec<u8>> {
    if let Some(text) = &file.content {
        return Some(text.as_bytes().to_vec());
    }
    if let Some(encoded) = &file.content_base64 {
        use base64::Engine as _;
        return base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok();
    }
    None
}

/// Map collected mirror files into workspace-relative proposal drafts.
pub fn output_map_to_drafts(
    files: &OutputFileMap,
    output_proposal_target: &str,
    projection_id: &str,
) -> Vec<LatticeProposalDraft> {
    let prefix = normalize_prefix(output_proposal_target);
    let mut drafts = Vec::with_capacity(files.len());
    for file in files.values() {
        let relative = strip_output_prefix(&file.path);
        let resource_path = join_proposal_path(&prefix, relative);
        let kind = classify_content(&file.content);
        drafts.push(LatticeProposalDraft {
            summary: format!(
                "Create resource {resource_path} from Cell projection {projection_id}"
            ),
            resource_path,
            content: file.content.clone(),
            kind,
        });
    }
    drafts
}

/// Run a celld projection loop and propose collected output via latticed.
pub async fn run_cell_task_and_propose<H: CelldHttpClient>(
    celld: &CelldClient<H>,
    lattice: &LatticeToolClient,
    workspace: &WorkspaceBinding,
    request: &ProjectionRunRequest,
    output_proposal_target: &str,
    provenance: &CellProposalProvenance,
) -> Result<(ProjectionRunResult, Vec<Value>), CellHostError> {
    let run_result = celld.run_projection(request)?;

    let drafts = output_map_to_drafts(
        &run_result.output_files,
        output_proposal_target,
        &run_result.projection_id,
    );
    let proposals = propose_output_drafts_with_provenance(
        lattice,
        workspace,
        &drafts,
        Some(provenance),
    )
    .await?;
    Ok((run_result, proposals))
}

fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim().trim_matches('/');
    if trimmed.is_empty() {
        "output".to_string()
    } else {
        trimmed.to_string()
    }
}

fn strip_output_prefix(collected_path: &str) -> &str {
    collected_path
        .strip_prefix("output/")
        .or_else(|| collected_path.strip_prefix("/output/"))
        .unwrap_or(collected_path)
        .trim_start_matches('/')
}

fn join_proposal_path(prefix: &str, relative: &str) -> String {
    let rel = relative.trim_start_matches('/');
    if prefix.is_empty() {
        rel.to_string()
    } else {
        format!("{prefix}/{rel}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernelfs::ContentKind;
    use lattice_cell_client::OutputFile;

    #[test]
    fn maps_collected_output_to_proposal_drafts() {
        let mut files = OutputFileMap::new();
        files.insert(
            "output/out.txt".into(),
            OutputFile {
                path: "output/out.txt".into(),
                sha256: "abc".into(),
                bytes: 5,
                content: b"hello".to_vec(),
            },
        );
        let drafts = output_map_to_drafts(&files, "Reports", "proj_1");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].resource_path, "Reports/out.txt");
        assert_eq!(drafts[0].content, b"hello");
        assert_eq!(drafts[0].kind, ContentKind::Text);
        assert!(drafts[0].summary.contains("proj_1"));
    }

    #[test]
    fn maps_binary_collected_output_as_bytes_kind() {
        let mut files = OutputFileMap::new();
        files.insert(
            "output/data.bin".into(),
            OutputFile {
                path: "output/data.bin".into(),
                sha256: "dead".into(),
                bytes: 4,
                content: vec![0x00, 0x01, 0xfe, 0xff],
            },
        );
        let drafts = output_map_to_drafts(&files, "Artifacts", "proj_bin");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].resource_path, "Artifacts/data.bin");
        assert_eq!(drafts[0].kind, ContentKind::Bytes);
    }

    #[test]
    fn cell_provenance_source_resource() {
        let prov = CellProposalProvenance {
            cell_id: "cell_demo".into(),
            projection_id: "proj_demo".into(),
            task_id: "task_1".into(),
            output_proposal_target: "Reports".into(),
            hydration_inputs: vec![HydrationInputDigest {
                path: "hello.txt".into(),
                content_hash: "abc".into(),
                resource_id: None,
            }],
        };
        assert_eq!(prov.source_resource(), "cell://cell_demo/proj_demo");
        let summary = prov.enrich_summary("Create resource Reports/out.txt");
        assert!(summary.contains("cellId=cell_demo"));
        assert!(summary.contains("projection=proj_demo"));
        assert!(summary.contains("hello.txt@abc"));
    }
}
