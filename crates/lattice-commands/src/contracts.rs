//! Shared IPC contracts for command-side effects beyond the undo journal.
//!
//! - **Commands** ([`crate::Command`], [`crate::Transaction`]): semantic mutations
//!   recorded in `.lattice/history.sqlite` for undo/redo and audit.
//! - **Executions** ([`ExecutionResult`]): long-running jobs (tasks, workflows)
//!   with captured stdout/stderr and materialized outputs.
//! - **Proposals** ([`TransactionProposal`]): reviewable bundles of commands
//!   produced by tasks, MCP, or external agents before they are applied.

use serde::{Deserialize, Serialize};

use crate::Command;

// Re-export the shared binding contract so IPC consumers can depend on
// `lattice_commands::BindingSpec` alongside ExecutionResult / proposals.
pub use lattice_data::BindingSpec;

/// A workspace resource produced or updated by an execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceOutput {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// Lifecycle state of a tracked execution (task run, workflow step, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// Captured result of a long-running execution for desktop / daemon IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    pub id: String,
    pub status: ExecutionStatus,
    pub stdout: String,
    pub stderr: String,
    /// ISO-8601 timestamp when the execution started.
    pub started_at: String,
    /// ISO-8601 timestamp when the execution finished, if terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub outputs: Vec<ResourceOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
}

/// Origin of a [`TransactionProposal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalSourceType {
    Task,
    Workflow,
    Artifact,
    Mcp,
    External,
}

/// Where a proposal was produced and which resource anchors it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalSource {
    #[serde(rename = "type")]
    pub source_type: ProposalSourceType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
}

/// Lifecycle state of a persisted [`TransactionProposal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProposalStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
}

fn is_pending(status: &ProposalStatus) -> bool {
    matches!(status, ProposalStatus::Pending)
}

/// A reviewable bundle of semantic commands before application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProposal {
    pub id: String,
    pub source: ProposalSource,
    pub summary: String,
    pub commands: Vec<Command>,
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    /// ISO-8601 timestamp when the proposal was created.
    pub created_at: String,
    /// Review lifecycle; omitted in older payloads defaults to pending.
    #[serde(default, skip_serializing_if = "is_pending")]
    pub status: ProposalStatus,
}

/// Inbox row for a persisted proposal (no command payloads).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProposalSummary {
    pub id: String,
    pub source: ProposalSource,
    pub summary: String,
    pub command_count: usize,
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub created_at: String,
    pub status: ProposalStatus,
}

impl TransactionProposal {
    /// Compact list row derived from a full proposal.
    pub fn summary(&self) -> TransactionProposalSummary {
        TransactionProposalSummary {
            id: self.id.clone(),
            source: self.source.clone(),
            summary: self.summary.clone(),
            command_count: self.commands.len(),
            affected_paths: self.affected_paths.clone(),
            warnings: self.warnings.clone(),
            created_at: self.created_at.clone(),
            status: self.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn execution_result_json_round_trip() {
        let result = ExecutionResult {
            id: "exec-42".into(),
            status: ExecutionStatus::Succeeded,
            stdout: "done\n".into(),
            stderr: String::new(),
            started_at: "2026-07-21T16:00:00Z".into(),
            finished_at: Some("2026-07-21T16:00:05Z".into()),
            outputs: vec![ResourceOutput {
                path: "Notes/Output.md".into(),
                kind: Some("page".into()),
                hash: Some("sha256:abc123".into()),
            }],
            proposal_id: Some("prop-7".into()),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"startedAt\""));
        assert!(json.contains("\"finishedAt\""));
        assert!(json.contains("\"proposalId\""));
        assert_eq!(
            serde_json::from_str::<ExecutionResult>(&json).unwrap(),
            result
        );
    }

    #[test]
    fn transaction_proposal_json_round_trip_with_page_create() {
        let proposal = TransactionProposal {
            id: "prop-1".into(),
            source: ProposalSource {
                source_type: ProposalSourceType::Task,
                resource: Some("tasks/hello.task".into()),
            },
            summary: "Create welcome page".into(),
            commands: vec![Command::PageCreate {
                path: PathBuf::from("Notes/Welcome.md"),
                content: "# Hello".into(),
            }],
            affected_paths: vec!["Notes/Welcome.md".into()],
            warnings: vec!["dry-run only".into()],
            created_at: "2026-07-21T16:30:00Z".into(),
            status: ProposalStatus::Pending,
        };

        let json = serde_json::to_string(&proposal).unwrap();
        assert!(json.contains("\"affectedPaths\""));
        assert!(json.contains("\"type\":\"page-create\""));
        // Pending status is omitted (additive default).
        assert!(!json.contains("\"status\""));
        assert_eq!(
            serde_json::from_str::<TransactionProposal>(&json).unwrap(),
            proposal
        );
    }

    #[test]
    fn transaction_proposal_status_defaults_when_absent() {
        let json = r#"{
            "id":"prop-2",
            "source":{"type":"external"},
            "summary":"Legacy",
            "commands":[],
            "affectedPaths":[],
            "createdAt":"2026-07-21T16:30:00Z"
        }"#;
        let proposal: TransactionProposal = serde_json::from_str(json).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Pending);
        assert_eq!(proposal.summary().command_count, 0);
    }

    #[test]
    fn binding_spec_json_round_trip_via_commands() {
        let binding = BindingSpec::SavedView {
            resource: "CRM.data".into(),
            view: "Board".into(),
        };
        let json = serde_json::to_string(&binding).unwrap();
        assert!(json.contains("\"type\":\"saved-view\""));
        assert_eq!(
            serde_json::from_str::<BindingSpec>(&json).unwrap(),
            binding
        );
    }
}
