//! celld guest hydrate → run → collect → Lattice proposal drafts (ADR 0063).
//!
//! Maps [`lattice_cell_client::OutputFileMap`] into KernelFS [`LatticeProposalDraft`]
//! values and pushes them through the same `propose_resource` path as WASI guests.

use kernelfs::{classify_content, LatticeProposalDraft};
use lattice_cell_client::{
    CelldClient, CelldHttpClient, OutputFileMap, ProjectionRunRequest, ProjectionRunResult,
};
use serde_json::Value;

use thiserror::Error;

use crate::lattice_client::LatticeToolClient;
use crate::wasi_host::{
    propose_output_drafts_with_provenance, DraftProvenance, ProposeDraftsError, WorkspaceBinding,
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
}

impl DraftProvenance for CellProposalProvenance {
    fn source_resource(&self) -> String {
        format!("cell://{}/{}", self.cell_id, self.projection_id)
    }

    fn enrich_summary(&self, base: &str) -> String {
        format!(
            "{base} [cell cellId={} projection={} task={} target={}]",
            self.cell_id, self.projection_id, self.task_id, self.output_proposal_target
        )
    }
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
        };
        assert_eq!(prov.source_resource(), "cell://cell_demo/proj_demo");
        let summary = prov.enrich_summary("Create resource Reports/out.txt");
        assert!(summary.contains("cellId=cell_demo"));
        assert!(summary.contains("projection=proj_demo"));
    }
}
