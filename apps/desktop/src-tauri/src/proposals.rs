//! Tauri commands for general transaction proposal review (ADR 0018).
//!
//! Link-repair proposals stay in `link_repair.rs` / `.lattice/link-repair/`.

use std::path::Path;

use lattice_commands::{
    apply_proposal, create_proposal, dismiss_proposal, load_proposal, list_proposal_summaries,
    Command as SemanticCommand, ProposalSource, ProposalSourceType, TransactionProposal,
    TransactionProposalSummary,
};
use serde::Deserialize;

use crate::commands::command_error_to_string;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProposalInput {
    pub summary: String,
    pub commands: Vec<SemanticCommand>,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub source_type: Option<ProposalSourceType>,
    #[serde(default)]
    pub source_resource: Option<String>,
}

#[tauri::command]
pub fn create_proposal_cmd(
    root: String,
    proposal: CreateProposalInput,
) -> Result<TransactionProposal, String> {
    let stored = create_proposal(
        Path::new(&root),
        TransactionProposal {
            id: String::new(),
            source: ProposalSource {
                source_type: proposal.source_type.unwrap_or(ProposalSourceType::External),
                resource: proposal.source_resource,
            },
            summary: proposal.summary,
            commands: proposal.commands,
            affected_paths: proposal.affected_paths,
            warnings: proposal.warnings,
            created_at: String::new(),
            status: Default::default(),
        },
    )
    .map_err(command_error_to_string)?;
    Ok(stored)
}

#[tauri::command]
pub fn get_proposal(root: String, proposal_id: String) -> Result<TransactionProposal, String> {
    load_proposal(Path::new(&root), &proposal_id).map_err(command_error_to_string)
}

#[tauri::command]
pub fn list_proposals(root: String) -> Result<Vec<TransactionProposalSummary>, String> {
    list_proposal_summaries(Path::new(&root)).map_err(command_error_to_string)
}

#[tauri::command]
pub fn dismiss_proposal_cmd(root: String, proposal_id: String) -> Result<(), String> {
    dismiss_proposal(Path::new(&root), &proposal_id).map_err(command_error_to_string)
}

#[tauri::command]
pub fn apply_proposal_cmd(
    root: String,
    proposal_id: String,
    selected_command_indices: Vec<usize>,
) -> Result<(), String> {
    apply_proposal(Path::new(&root), &proposal_id, &selected_command_indices)
        .map_err(command_error_to_string)
}

/// Seed a demo page-create proposal for manual / smoke testing.
#[tauri::command]
pub fn create_demo_proposal(root: String) -> Result<TransactionProposal, String> {
    create_proposal_cmd(
        root,
        CreateProposalInput {
            summary: "Demo: create Proposals/Welcome.md".into(),
            commands: vec![SemanticCommand::PageCreate {
                path: "Proposals/Welcome.md".into(),
                content: "# Welcome from a proposal\n\nAccepted through the review inbox.\n".into(),
            }],
            affected_paths: vec!["Proposals/Welcome.md".into()],
            warnings: vec!["Demo seed — safe to reject.".into()],
            source_type: Some(ProposalSourceType::External),
            source_resource: None,
        },
    )
}
