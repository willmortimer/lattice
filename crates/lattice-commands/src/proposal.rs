//! Persist reviewable [`TransactionProposal`] bundles under `.lattice/proposals/`.
//!
//! This is the general agent/task proposal store (ADR 0018). Link-repair keeps
//! its own sibling directory and is not migrated here.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_core::OPERATIONAL_DIR;

use crate::command::{Command, Transaction};
use crate::contracts::{ProposalStatus, TransactionProposal, TransactionProposalSummary};
use crate::engine::CommandEngine;
use crate::{Error, Result};

pub const PROPOSALS_DIR: &str = "proposals";

/// Directory holding deferred transaction proposals: `<workspace>/.lattice/proposals/`.
pub fn proposals_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(OPERATIONAL_DIR).join(PROPOSALS_DIR)
}

fn proposal_path(workspace_root: &Path, id: &str) -> PathBuf {
    proposals_dir(workspace_root).join(format!("{id}.json"))
}

/// Fresh proposal id for a new reviewable bundle.
pub fn new_proposal_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Current UTC timestamp as ISO-8601 (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn proposal_now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_unix_secs_iso(secs)
}

fn format_unix_secs_iso(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Algorithms from Howard Hinnant's `civil_from_days` (proleptic Gregorian).
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

/// Persist a proposal (create or overwrite). Forces `status` to pending on save
/// of a new reviewable bundle when callers leave the default.
pub fn save_proposal(workspace_root: &Path, proposal: &TransactionProposal) -> Result<()> {
    let dir = proposals_dir(workspace_root);
    fs::create_dir_all(&dir).map_err(|source| Error::io(&dir, source))?;
    let path = proposal_path(workspace_root, &proposal.id);
    let payload = serde_json::to_string_pretty(proposal)?;
    fs::write(&path, payload).map_err(|source| Error::io(&path, source))
}

/// Create and persist a proposal, assigning id/created_at when empty.
pub fn create_proposal(
    workspace_root: &Path,
    mut proposal: TransactionProposal,
) -> Result<TransactionProposal> {
    if proposal.id.trim().is_empty() {
        proposal.id = new_proposal_id();
    }
    if proposal.created_at.trim().is_empty() {
        proposal.created_at = proposal_now_iso();
    }
    proposal.status = ProposalStatus::Pending;
    if proposal.affected_paths.is_empty() {
        proposal.affected_paths = affected_paths_from_commands(&proposal.commands);
    }
    save_proposal(workspace_root, &proposal)?;
    Ok(proposal)
}

fn affected_paths_from_commands(commands: &[Command]) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for command in commands {
        for path in command.touched_paths() {
            paths.insert(path.display().to_string());
        }
    }
    paths.into_iter().collect()
}

/// Load one persisted proposal by id.
pub fn load_proposal(workspace_root: &Path, id: &str) -> Result<TransactionProposal> {
    let path = proposal_path(workspace_root, id);
    let payload = fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
    serde_json::from_str(&payload).map_err(Error::from)
}

/// List summaries of pending proposals, newest first.
pub fn list_proposal_summaries(workspace_root: &Path) -> Result<Vec<TransactionProposalSummary>> {
    let dir = proposals_dir(workspace_root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut summaries = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|source| Error::io(&dir, source))? {
        let entry = entry.map_err(|source| Error::io(&dir, source))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let payload = fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
        let proposal: TransactionProposal = serde_json::from_str(&payload)?;
        if proposal.status != ProposalStatus::Pending {
            continue;
        }
        summaries.push(proposal.summary());
    }
    summaries.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(summaries)
}

/// Remove a persisted proposal without applying it (reject / dismiss).
pub fn dismiss_proposal(workspace_root: &Path, id: &str) -> Result<()> {
    let path = proposal_path(workspace_root, id);
    if path.is_file() {
        fs::remove_file(&path).map_err(|source| Error::io(&path, source))?;
    }
    Ok(())
}

/// Build a transaction from the selected command indices (order preserved).
pub fn build_proposal_transaction(
    proposal: &TransactionProposal,
    selected_indices: &[usize],
) -> Result<Transaction> {
    if selected_indices.is_empty() {
        return Err(Error::InvalidResourceTarget {
            path: PathBuf::from(".lattice/proposals"),
            reason: "accept requires at least one command index".into(),
        });
    }
    let mut seen = BTreeSet::new();
    let mut commands = Vec::with_capacity(selected_indices.len());
    for &index in selected_indices {
        if !seen.insert(index) {
            continue;
        }
        let Some(command) = proposal.commands.get(index) else {
            return Err(Error::InvalidResourceTarget {
                path: PathBuf::from(".lattice/proposals"),
                reason: format!(
                    "command index {index} is out of range (0..{})",
                    proposal.commands.len()
                ),
            });
        };
        commands.push(command.clone());
    }
    Ok(Transaction::new(proposal.summary.clone(), commands))
}

/// Apply selected commands through [`CommandEngine`], then remove the proposal.
pub fn apply_proposal(
    workspace_root: &Path,
    id: &str,
    selected_indices: &[usize],
) -> Result<()> {
    let proposal = load_proposal(workspace_root, id)?;
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::InvalidResourceTarget {
            path: PathBuf::from(".lattice/proposals").join(format!("{id}.json")),
            reason: format!("proposal is not pending (status={:?})", proposal.status),
        });
    }
    let tx = build_proposal_transaction(&proposal, selected_indices)?;
    let mut engine = CommandEngine::open(workspace_root)?;
    engine.apply(tx)?;
    dismiss_proposal(workspace_root, id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{ProposalSource, ProposalSourceType};
    use lattice_core::Workspace;
    use tempfile::TempDir;

    fn workspace() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Proposals").unwrap();
        dir
    }

    fn demo_proposal(id: &str, path: &str) -> TransactionProposal {
        TransactionProposal {
            id: id.into(),
            source: ProposalSource {
                source_type: ProposalSourceType::Task,
                resource: Some("tasks/demo.task".into()),
            },
            summary: format!("Create {path}"),
            commands: vec![Command::PageCreate {
                path: PathBuf::from(path),
                content: format!("# {path}\n"),
            }],
            affected_paths: vec![path.into()],
            warnings: vec![],
            created_at: "2026-07-21T17:00:00Z".into(),
            status: ProposalStatus::Pending,
        }
    }

    #[test]
    fn format_unix_secs_iso_known_instant() {
        // 2026-07-21T16:00:00Z
        assert_eq!(format_unix_secs_iso(1_784_649_600), "2026-07-21T16:00:00Z");
    }

    #[test]
    fn create_list_get_dismiss_round_trip() {
        let dir = workspace();
        let created = create_proposal(dir.path(), demo_proposal("", "Notes/A.md")).unwrap();
        assert!(!created.id.is_empty());
        assert_eq!(created.status, ProposalStatus::Pending);

        let listed = list_proposal_summaries(dir.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].command_count, 1);

        let loaded = load_proposal(dir.path(), &created.id).unwrap();
        assert_eq!(loaded.summary, created.summary);

        dismiss_proposal(dir.path(), &created.id).unwrap();
        assert!(list_proposal_summaries(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn apply_subset_writes_and_undo_restores() {
        let dir = workspace();
        let proposal = TransactionProposal {
            id: "multi".into(),
            source: ProposalSource {
                source_type: ProposalSourceType::External,
                resource: None,
            },
            summary: "Create two pages".into(),
            commands: vec![
                Command::PageCreate {
                    path: PathBuf::from("Notes/One.md"),
                    content: "# One\n".into(),
                },
                Command::PageCreate {
                    path: PathBuf::from("Notes/Two.md"),
                    content: "# Two\n".into(),
                },
            ],
            affected_paths: vec!["Notes/One.md".into(), "Notes/Two.md".into()],
            warnings: vec!["demo".into()],
            created_at: "2026-07-21T17:05:00Z".into(),
            status: ProposalStatus::Pending,
        };
        save_proposal(dir.path(), &proposal).unwrap();

        // Accept only the first command.
        apply_proposal(dir.path(), "multi", &[0]).unwrap();
        assert!(dir.path().join("Notes/One.md").exists());
        assert!(!dir.path().join("Notes/Two.md").exists());
        assert!(list_proposal_summaries(dir.path()).unwrap().is_empty());

        let mut engine = CommandEngine::open(dir.path()).unwrap();
        let undone = engine.undo().unwrap().expect("undo");
        assert_eq!(undone.transaction_id.is_empty(), false);
        assert!(!dir.path().join("Notes/One.md").exists());
    }

    #[test]
    fn apply_rejects_out_of_range_index() {
        let dir = workspace();
        save_proposal(dir.path(), &demo_proposal("p1", "Notes/X.md")).unwrap();
        let err = apply_proposal(dir.path(), "p1", &[3]).unwrap_err();
        assert!(err.to_string().contains("out of range"));
        // Proposal remains pending.
        assert_eq!(list_proposal_summaries(dir.path()).unwrap().len(), 1);
    }

    #[test]
    fn proposals_dir_is_sibling_to_link_repair() {
        let dir = workspace();
        let path = proposals_dir(dir.path());
        assert!(path.ends_with(".lattice/proposals"));
        assert!(!path.ends_with("link-repair"));
    }

    #[test]
    fn deserializes_python_sdk_sample_proposal_json() {
        let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/lattice-py/testdata/sample_proposal.json");
        let payload = fs::read_to_string(&sample)
            .unwrap_or_else(|err| panic!("missing SDK sample at {}: {err}", sample.display()));
        let proposal: TransactionProposal = serde_json::from_str(&payload).unwrap();
        assert_eq!(proposal.id, "00000000-0000-4000-8000-000000000001");
        assert_eq!(proposal.source.source_type, ProposalSourceType::Task);
        assert_eq!(
            proposal.source.resource.as_deref(),
            Some("Tasks/ProposePage.task")
        );
        assert_eq!(proposal.commands.len(), 1);
        match &proposal.commands[0] {
            Command::PageCreate { path, content } => {
                assert_eq!(path, &PathBuf::from("Notes/SdkSample.md"));
                assert!(content.contains("Python SDK"));
            }
            other => panic!("expected PageCreate, got {other:?}"),
        }
        assert_eq!(proposal.affected_paths, vec!["Notes/SdkSample.md"]);
        assert_eq!(proposal.status, ProposalStatus::Pending);
    }
}
