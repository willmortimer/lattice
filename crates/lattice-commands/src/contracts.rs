//! Shared IPC contracts for command-side effects beyond the undo journal.
//!
//! - **Commands** ([`crate::Command`], [`crate::Transaction`]): semantic mutations
//!   recorded in `.lattice/history.sqlite` for undo/redo and audit.
//! - **Executions** ([`ExecutionResult`]): long-running jobs (tasks, workflows)
//!   with captured stdout/stderr and materialized outputs.
//! - **Proposals** ([`TransactionProposal`]): reviewable bundles of commands
//!   produced by tasks, MCP, or external agents before they are applied.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Command;

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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };

        let json = serde_json::to_string(&proposal).unwrap();
        assert!(json.contains("\"affectedPaths\""));
        assert!(json.contains("\"type\":\"page-create\""));
        assert_eq!(
            serde_json::from_str::<TransactionProposal>(&json).unwrap(),
            proposal
        );
    }
}
