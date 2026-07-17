//! Compose rename/move + span-precise page updates into one transaction, and
//! persist deferred external-rename repair proposals under `.lattice/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use lattice_core::{
    apply_span_replacements, LinkRepairCandidate, LinkRepairPlan, LinkRepairProposalSummary,
    OPERATIONAL_DIR,
};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};

use crate::command::{Command, Transaction};
use crate::history::unix_now;
use crate::{Error, Result};

pub const LINK_REPAIR_DIR: &str = "link-repair";

/// Directory holding deferred repair proposals: `<workspace>/.lattice/link-repair/`.
pub fn proposals_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(OPERATIONAL_DIR).join(LINK_REPAIR_DIR)
}

fn proposal_path(workspace_root: &Path, id: &str) -> PathBuf {
    proposals_dir(workspace_root).join(format!("{id}.json"))
}

/// Persist a deferred repair proposal (external rename path).
pub fn save_link_repair_proposal(workspace_root: &Path, plan: &LinkRepairPlan) -> Result<()> {
    let dir = proposals_dir(workspace_root);
    fs::create_dir_all(&dir).map_err(|source| Error::io(&dir, source))?;
    let path = proposal_path(workspace_root, &plan.id);
    let payload = serde_json::to_string_pretty(plan)?;
    fs::write(&path, payload).map_err(|source| Error::io(&path, source))
}

/// Load one persisted proposal by id.
pub fn load_link_repair_proposal(workspace_root: &Path, id: &str) -> Result<LinkRepairPlan> {
    let path = proposal_path(workspace_root, id);
    let payload = fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
    serde_json::from_str(&payload).map_err(Error::from)
}

/// List summaries of all persisted proposals, newest first.
pub fn list_link_repair_proposals(workspace_root: &Path) -> Result<Vec<LinkRepairProposalSummary>> {
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
        let plan: LinkRepairPlan = serde_json::from_str(&payload)?;
        summaries.push(plan.summary());
    }
    summaries.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(summaries)
}

/// Remove a persisted proposal without applying it.
pub fn dismiss_link_repair_proposal(workspace_root: &Path, id: &str) -> Result<()> {
    let path = proposal_path(workspace_root, id);
    if path.is_file() {
        fs::remove_file(&path).map_err(|source| Error::io(&path, source))?;
    }
    Ok(())
}

/// Build `PageUpdate` commands for accepted repair candidates.
pub fn build_link_repair_page_updates(
    store: &NativeWorkspaceStore,
    plan: &LinkRepairPlan,
    accepted_candidate_ids: &[String],
) -> Result<Vec<Command>> {
    let accepted: BTreeSet<&str> = accepted_candidate_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut by_source: BTreeMap<PathBuf, Vec<&LinkRepairCandidate>> = BTreeMap::new();
    for candidate in &plan.candidates {
        if !accepted.contains(candidate.id.as_str()) {
            continue;
        }
        by_source
            .entry(candidate.occurrence.source_path.clone())
            .or_default()
            .push(candidate);
    }

    let mut commands = Vec::new();
    for (source_path, candidates) in by_source {
        let bytes = store.read(&source_path)?;
        let content = String::from_utf8(bytes).map_err(|_| Error::InvalidResourceTarget {
            path: source_path.clone(),
            reason: "link repair requires UTF-8 page content".into(),
        })?;
        let replacements: Vec<(usize, usize, &str)> = candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.occurrence.source_start_byte,
                    candidate.occurrence.source_end_byte,
                    candidate.new_text.as_str(),
                )
            })
            .collect();
        let updated = apply_span_replacements(&content, &replacements).ok_or_else(|| {
            Error::InvalidResourceTarget {
                path: source_path.clone(),
                reason: "link repair span is out of bounds or not on a char boundary".into(),
            }
        })?;
        let meta = store
            .metadata(&source_path)
            .map_err(Error::from)?
            .revision;
        commands.push(Command::PageUpdate {
            path: source_path,
            content: updated,
            base_revision: meta.hash,
        });
    }
    Ok(commands)
}

/// Compose an optional rename/move command with accepted page updates.
pub fn build_link_repair_transaction(
    store: &NativeWorkspaceStore,
    rename_command: Option<Command>,
    plan: &LinkRepairPlan,
    accepted_candidate_ids: &[String],
    summary: impl Into<String>,
) -> Result<Transaction> {
    let mut commands = Vec::new();
    if let Some(rename) = rename_command {
        commands.push(rename);
    }
    commands.extend(build_link_repair_page_updates(
        store,
        plan,
        accepted_candidate_ids,
    )?);
    Ok(Transaction::new(summary, commands))
}

/// Convenience for external renames: build and persist a proposal when links
/// would be affected.
pub fn maybe_save_external_link_repair_proposal(
    workspace_root: &Path,
    plan: &LinkRepairPlan,
) -> Result<Option<LinkRepairProposalSummary>> {
    if plan.candidates.is_empty() {
        return Ok(None);
    }
    save_link_repair_proposal(workspace_root, plan)?;
    Ok(Some(plan.summary()))
}

/// Fresh plan id for a new repair review.
pub fn new_link_repair_plan_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Current unix timestamp for plan metadata.
pub fn link_repair_now() -> u64 {
    unix_now().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::{
        build_link_repair_plan, LinkOccurrence, LinkRepairSource, LinkRepairStatus,
        MarkdownLinkKind, Resource, ResourceCatalog, ResourceKind, Workspace,
    };
    use tempfile::TempDir;

    fn workspace() -> (TempDir, NativeWorkspaceStore) {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Link Repair").unwrap();
        let store = NativeWorkspaceStore::new(dir.path());
        (dir, store)
    }

    fn write_page(dir: &TempDir, path: &str, content: &str) {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    fn plan_with_candidates(dir: &TempDir) -> LinkRepairPlan {
        write_page(dir, "Notes/Home.md", "See [[Other]] and [[Other|label]].\n");
        write_page(dir, "Notes/Other.md", "# Other\n");
        let catalog = ResourceCatalog::new(&[
            Resource {
                path: "Notes/Home.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Notes/Other.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Notes/Renamed.md".into(),
                kind: ResourceKind::Page,
            },
        ]);
        let occurrences = vec![LinkOccurrence {
            source_path: "Notes/Home.md".into(),
            kind: MarkdownLinkKind::Wiki,
            raw_target: "Other".into(),
            anchor: None,
            label: None,
            source_start_byte: 4,
            source_end_byte: 12,
            source_start_line: 1,
            source_start_column: 5,
            source_end_line: 1,
            source_end_column: 13,
        }];
        build_link_repair_plan(
            &catalog,
            occurrences,
            "Notes/Other.md".into(),
            "Notes/Renamed.md".into(),
            LinkRepairSource::LatticeRename,
            "plan-1",
            link_repair_now(),
        )
    }

    #[test]
    fn page_updates_rewrite_spans_from_end_of_file() {
        let (dir, store) = workspace();
        write_page(&dir, "Notes/Home.md", "[[A]] then [[B]]\n");
        write_page(&dir, "Notes/B.md", "# B\n");
        let plan = LinkRepairPlan {
            id: "p1".into(),
            rename_from: "Notes/B.md".into(),
            rename_to: "Notes/C.md".into(),
            source: LinkRepairSource::LatticeRename,
            created_at: 1,
            candidates: vec![
                LinkRepairCandidate {
                    id: "p1-0".into(),
                    occurrence: LinkOccurrence {
                        source_path: "Notes/Home.md".into(),
                        kind: MarkdownLinkKind::Wiki,
                        raw_target: "A".into(),
                        anchor: None,
                        label: None,
                        source_start_byte: 0,
                        source_end_byte: 5,
                        source_start_line: 1,
                        source_start_column: 1,
                        source_end_line: 1,
                        source_end_column: 6,
                    },
                    old_target: "A".into(),
                    new_target: "C".into(),
                    new_text: "[[C]]".into(),
                    status: LinkRepairStatus::Resolved,
                    ambiguity: None,
                },
                LinkRepairCandidate {
                    id: "p1-1".into(),
                    occurrence: LinkOccurrence {
                        source_path: "Notes/Home.md".into(),
                        kind: MarkdownLinkKind::Wiki,
                        raw_target: "B".into(),
                        anchor: None,
                        label: None,
                        source_start_byte: 11,
                        source_end_byte: 16,
                        source_start_line: 1,
                        source_start_column: 12,
                        source_end_line: 1,
                        source_end_column: 17,
                    },
                    old_target: "B".into(),
                    new_target: "C".into(),
                    new_text: "[[C]]".into(),
                    status: LinkRepairStatus::Resolved,
                    ambiguity: None,
                },
            ],
        };
        let updates = build_link_repair_page_updates(
            &store,
            &plan,
            &["p1-0".into(), "p1-1".into()],
        )
        .unwrap();
        assert_eq!(updates.len(), 1);
        let Command::PageUpdate { content, .. } = &updates[0] else {
            panic!("expected page update");
        };
        assert_eq!(content, "[[C]] then [[C]]\n");
    }

    #[test]
    fn transaction_composes_rename_and_page_updates() {
        let (dir, store) = workspace();
        let plan = plan_with_candidates(&dir);
        let tx = build_link_repair_transaction(
            &store,
            Some(Command::ResourceRename {
                from: plan.rename_from.clone(),
                to: plan.rename_to.clone(),
            }),
            &plan,
            &[plan.candidates[0].id.clone()],
            "Rename with link repair",
        )
        .unwrap();
        assert_eq!(tx.commands.len(), 2);
        let mut engine = crate::CommandEngine::open(dir.path()).unwrap();
        engine.apply(tx).unwrap();
        let home = std::fs::read_to_string(dir.path().join("Notes/Home.md")).unwrap();
        assert!(home.contains("[[Renamed]]"));
        assert!(!dir.path().join("Notes/Other.md").exists());
        assert!(dir.path().join("Notes/Renamed.md").exists());
    }

    #[test]
    fn proposal_round_trip_and_dismiss() {
        let (dir, _) = workspace();
        let plan = plan_with_candidates(&dir);
        save_link_repair_proposal(dir.path(), &plan).unwrap();
        let listed = list_link_repair_proposals(dir.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, plan.id);
        let loaded = load_link_repair_proposal(dir.path(), &plan.id).unwrap();
        assert_eq!(loaded, plan);
        dismiss_link_repair_proposal(dir.path(), &plan.id).unwrap();
        assert!(list_link_repair_proposals(dir.path()).unwrap().is_empty());
    }
}
