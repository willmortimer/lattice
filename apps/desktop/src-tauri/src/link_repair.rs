//! Tauri commands for reviewed link repair after resource renames.

use std::path::{Path, PathBuf};

use lattice_commands::{
    build_batch_link_repair_plan, build_batch_link_repair_transaction, build_link_repair_transaction,
    dismiss_link_repair_proposal, link_repair_now, list_link_repair_proposals,
    load_link_repair_proposal, new_link_repair_plan_id, save_link_repair_proposal,
    Command as SemanticCommand, CommandEngine, Transaction,
};
use lattice_core::{
    BatchLinkRepairPlan, LinkRepairPlan, LinkRepairProposalSummary, LinkRepairSource,
};
use lattice_index::WorkspaceIndex;
use lattice_storage::NativeWorkspaceStore;
use serde::Deserialize;
use tauri::State;

use crate::commands::command_error_to_string;
use crate::resource_links::ResourceCatalogState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepairMoveInput {
    pub from: String,
    pub to: String,
}

#[tauri::command]
pub fn preview_link_repair(
    root: String,
    from: String,
    to: String,
    source: LinkRepairSource,
    catalog_state: State<ResourceCatalogState>,
) -> Result<LinkRepairPlan, String> {
    catalog_state.refresh(&root)?;
    let index = WorkspaceIndex::open(Path::new(&root)).map_err(|error| error.to_string())?;
    index
        .link_repair_plan(
            Path::new(&from),
            Path::new(&to),
            source,
            &new_link_repair_plan_id(),
            link_repair_now(),
        )
        .map_err(|error| error.to_string())
}

/// Preview link repair for multiple from→to path changes as one combined plan.
#[tauri::command]
pub fn preview_batch_link_repair(
    root: String,
    moves: Vec<LinkRepairMoveInput>,
    source: LinkRepairSource,
    catalog_state: State<ResourceCatalogState>,
) -> Result<BatchLinkRepairPlan, String> {
    catalog_state.refresh(&root)?;
    if moves.len() < 2 {
        return Err("Batch link repair requires at least two path changes.".into());
    }
    let index = WorkspaceIndex::open(Path::new(&root)).map_err(|error| error.to_string())?;
    let created_at = link_repair_now();
    let mut plans = Vec::with_capacity(moves.len());
    for change in &moves {
        let plan = index
            .link_repair_plan(
                Path::new(&change.from),
                Path::new(&change.to),
                source,
                &new_link_repair_plan_id(),
                created_at,
            )
            .map_err(|error| error.to_string())?;
        plans.push(plan);
    }
    Ok(build_batch_link_repair_plan(
        new_link_repair_plan_id(),
        created_at,
        source,
        plans,
    ))
}

#[tauri::command]
pub fn get_link_repair_proposal(root: String, proposal_id: String) -> Result<LinkRepairPlan, String> {
    load_link_repair_proposal(Path::new(&root), &proposal_id).map_err(command_error_to_string)
}

#[tauri::command]
pub fn list_link_repair_proposals_cmd(root: String) -> Result<Vec<LinkRepairProposalSummary>, String> {
    list_link_repair_proposals(Path::new(&root)).map_err(command_error_to_string)
}

#[tauri::command]
pub fn dismiss_link_repair_proposal_cmd(root: String, proposal_id: String) -> Result<(), String> {
    dismiss_link_repair_proposal(Path::new(&root), &proposal_id)
        .map_err(command_error_to_string)
}

#[tauri::command]
pub fn defer_link_repair_proposal(root: String, plan: LinkRepairPlan) -> Result<(), String> {
    save_link_repair_proposal(Path::new(&root), &plan).map_err(command_error_to_string)
}

#[tauri::command]
pub fn apply_link_repair(
    root: String,
    from: String,
    to: String,
    accepted_candidate_ids: Vec<String>,
    plan: LinkRepairPlan,
    catalog_state: State<ResourceCatalogState>,
) -> Result<(), String> {
    catalog_state.refresh(&root)?;
    let store = NativeWorkspaceStore::new(Path::new(&root));
    let summary = link_repair_transaction_summary(
        Path::new(&from),
        Path::new(&to),
        accepted_candidate_ids.len(),
    );
    let tx = build_link_repair_transaction(
        &store,
        Some(SemanticCommand::ResourceRename {
            from: PathBuf::from(from),
            to: PathBuf::from(to),
        }),
        &plan,
        &accepted_candidate_ids,
        summary,
    )
    .map_err(command_error_to_string)?;
    apply_transaction(&root, tx)?;
    dismiss_link_repair_proposal(Path::new(&root), &plan.id).ok();
    Ok(())
}

/// Apply N path changes plus accepted link repairs in one transaction.
#[tauri::command]
pub fn apply_batch_link_repair(
    root: String,
    moves: Vec<LinkRepairMoveInput>,
    accepted_candidate_ids: Vec<String>,
    plan: BatchLinkRepairPlan,
    catalog_state: State<ResourceCatalogState>,
) -> Result<(), String> {
    catalog_state.refresh(&root)?;
    if moves.len() < 2 {
        return Err("Batch link repair requires at least two path changes.".into());
    }
    let store = NativeWorkspaceStore::new(Path::new(&root));
    let path_moves: Vec<(PathBuf, PathBuf)> = moves
        .iter()
        .map(|change| (PathBuf::from(&change.from), PathBuf::from(&change.to)))
        .collect();
    let summary = batch_link_repair_transaction_summary(
        path_moves.len(),
        accepted_candidate_ids.len(),
    );
    let tx = build_batch_link_repair_transaction(
        &store,
        &path_moves,
        &plan.candidates,
        &accepted_candidate_ids,
        summary,
    )
    .map_err(command_error_to_string)?;
    apply_transaction(&root, tx)?;
    dismiss_link_repair_proposal(Path::new(&root), &plan.id).ok();
    Ok(())
}

#[tauri::command]
pub fn apply_link_repair_proposal(
    root: String,
    proposal_id: String,
    accepted_candidate_ids: Vec<String>,
    catalog_state: State<ResourceCatalogState>,
) -> Result<(), String> {
    catalog_state.refresh(&root)?;
    let plan = load_link_repair_proposal(Path::new(&root), &proposal_id)
        .map_err(command_error_to_string)?;
    let store = NativeWorkspaceStore::new(Path::new(&root));
    let tx = build_link_repair_transaction(
        &store,
        None,
        &plan,
        &accepted_candidate_ids,
        format!(
            "Repair {} link(s) after external rename of {}",
            accepted_candidate_ids.len(),
            plan.rename_from.display()
        ),
    )
    .map_err(command_error_to_string)?;
    apply_transaction(&root, tx)?;
    dismiss_link_repair_proposal(Path::new(&root), &proposal_id)
        .map_err(command_error_to_string)?;
    Ok(())
}

/// Build and persist a deferred proposal for an external rename.
pub fn save_external_link_repair_proposal(
    workspace_root: &Path,
    from: &Path,
    to: &Path,
) -> Result<Option<LinkRepairProposalSummary>, String> {
    let index = WorkspaceIndex::open(workspace_root).map_err(|error| error.to_string())?;
    let plan = index
        .link_repair_plan(
            from,
            to,
            LinkRepairSource::ExternalRename,
            &new_link_repair_plan_id(),
            link_repair_now(),
        )
        .map_err(|error| error.to_string())?;
    if plan.candidates.is_empty() {
        return Ok(None);
    }
    save_link_repair_proposal(workspace_root, &plan).map_err(command_error_to_string)?;
    Ok(Some(plan.summary()))
}

fn link_repair_transaction_summary(from: &Path, to: &Path, repair_count: usize) -> String {
    let action = if from.parent() == to.parent() {
        "Rename"
    } else {
        "Move"
    };
    format!(
        "{action} {} to {} with {repair_count} link repair(s)",
        from.display(),
        to.display()
    )
}

fn batch_link_repair_transaction_summary(move_count: usize, repair_count: usize) -> String {
    format!("Move {move_count} resources with {repair_count} link repair(s)")
}

fn apply_transaction(root: &str, tx: Transaction) -> Result<(), String> {
    let mut engine = CommandEngine::open(Path::new(root)).map_err(command_error_to_string)?;
    engine.apply(tx).map_err(command_error_to_string)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::{LinkRepairStatus, ResourceKind, Workspace};

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Repair").unwrap();
        std::fs::create_dir_all(dir.path().join("Notes")).unwrap();
        std::fs::write(
            dir.path().join("Notes/Home.md"),
            "Link to [[Other]] here.\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Notes/Other.md"), "# Other\n").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index
            .upsert_resource(&lattice_core::Resource {
                path: "Notes/Home.md".into(),
                kind: ResourceKind::Page,
            })
            .unwrap();
        index
            .upsert_resource(&lattice_core::Resource {
                path: "Notes/Other.md".into(),
                kind: ResourceKind::Page,
            })
            .unwrap();
        dir
    }

    fn init_batch_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Batch Repair").unwrap();
        std::fs::create_dir_all(dir.path().join("Notes")).unwrap();
        std::fs::create_dir_all(dir.path().join("Archive")).unwrap();
        std::fs::write(
            dir.path().join("Notes/Home.md"),
            "See [[Alpha]] and [[Beta]].\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Notes/Alpha.md"), "# Alpha\n").unwrap();
        std::fs::write(dir.path().join("Notes/Beta.md"), "# Beta\n").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        for path in ["Notes/Home.md", "Notes/Alpha.md", "Notes/Beta.md"] {
            index
                .upsert_resource(&lattice_core::Resource {
                    path: path.into(),
                    kind: ResourceKind::Page,
                })
                .unwrap();
        }
        dir
    }

    #[test]
    fn external_proposal_persists_when_links_exist() {
        let dir = init_workspace();
        // Simulate watcher order: plan while inbound spans still target `from`,
        // then update index paths for the renamed file.
        let summary = save_external_link_repair_proposal(
            dir.path(),
            Path::new("Notes/Other.md"),
            Path::new("Notes/Renamed.md"),
        )
        .unwrap()
        .expect("expected proposal");
        assert_eq!(summary.candidate_count, 1);

        std::fs::rename(
            dir.path().join("Notes/Other.md"),
            dir.path().join("Notes/Renamed.md"),
        )
        .unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index
            .remove_resource(Path::new("Notes/Other.md"))
            .unwrap();
        index
            .upsert_resource(&lattice_core::Resource {
                path: "Notes/Renamed.md".into(),
                kind: ResourceKind::Page,
            })
            .unwrap();

        let listed = list_link_repair_proposals(dir.path()).unwrap();
        assert_eq!(listed.len(), 1);
    }

    #[test]
    fn link_repair_transaction_summary_distinguishes_rename_and_move() {
        assert_eq!(
            link_repair_transaction_summary(
                Path::new("Notes/A.md"),
                Path::new("Notes/B.md"),
                2,
            ),
            "Rename Notes/A.md to Notes/B.md with 2 link repair(s)"
        );
        assert_eq!(
            link_repair_transaction_summary(
                Path::new("Notes/A.md"),
                Path::new("Archive/A.md"),
                1,
            ),
            "Move Notes/A.md to Archive/A.md with 1 link repair(s)"
        );
    }

    #[test]
    fn apply_link_repair_transaction_rewrites_links() {
        let dir = init_workspace();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        let plan = index
            .link_repair_plan(
                Path::new("Notes/Other.md"),
                Path::new("Notes/Renamed.md"),
                LinkRepairSource::LatticeRename,
                &new_link_repair_plan_id(),
                link_repair_now(),
            )
            .unwrap();
        let accepted = plan
            .candidates
            .iter()
            .filter(|candidate| candidate.status == LinkRepairStatus::Resolved)
            .map(|candidate| candidate.id.clone())
            .collect::<Vec<_>>();
        let store = NativeWorkspaceStore::new(dir.path());
        let tx = build_link_repair_transaction(
            &store,
            Some(SemanticCommand::ResourceRename {
                from: PathBuf::from("Notes/Other.md"),
                to: PathBuf::from("Notes/Renamed.md"),
            }),
            &plan,
            &accepted,
            "repair",
        )
        .unwrap();
        apply_transaction(&dir.path().to_string_lossy(), tx).unwrap();
        let home = std::fs::read_to_string(dir.path().join("Notes/Home.md")).unwrap();
        assert!(home.contains("[[Renamed]]"));
    }

    #[test]
    fn batch_preview_and_apply_repairs_two_moves() {
        let dir = init_batch_workspace();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        let created_at = link_repair_now();
        let plans = vec![
            index
                .link_repair_plan(
                    Path::new("Notes/Alpha.md"),
                    Path::new("Archive/Alpha.md"),
                    LinkRepairSource::LatticeRename,
                    &new_link_repair_plan_id(),
                    created_at,
                )
                .unwrap(),
            index
                .link_repair_plan(
                    Path::new("Notes/Beta.md"),
                    Path::new("Archive/Beta.md"),
                    LinkRepairSource::LatticeRename,
                    &new_link_repair_plan_id(),
                    created_at,
                )
                .unwrap(),
        ];
        let mut batch = build_batch_link_repair_plan(
            new_link_repair_plan_id(),
            created_at,
            LinkRepairSource::LatticeRename,
            plans,
        );
        // Index may keep wiki titles stable across folder moves; inject observable rewrites.
        batch.candidates = vec![
            lattice_core::LinkRepairCandidate {
                id: format!("{}-0", batch.id),
                occurrence: lattice_core::LinkOccurrence {
                    source_path: "Notes/Home.md".into(),
                    kind: lattice_core::MarkdownLinkKind::Wiki,
                    raw_target: "Alpha".into(),
                    anchor: None,
                    label: None,
                    source_start_byte: 4,
                    source_end_byte: 13,
                    source_start_line: 1,
                    source_start_column: 5,
                    source_end_line: 1,
                    source_end_column: 14,
                },
                old_target: "Alpha".into(),
                new_target: "Archive/Alpha".into(),
                new_text: "[[Archive/Alpha]]".into(),
                status: LinkRepairStatus::Resolved,
                ambiguity: None,
            },
            lattice_core::LinkRepairCandidate {
                id: format!("{}-1", batch.id),
                occurrence: lattice_core::LinkOccurrence {
                    source_path: "Notes/Home.md".into(),
                    kind: lattice_core::MarkdownLinkKind::Wiki,
                    raw_target: "Beta".into(),
                    anchor: None,
                    label: None,
                    source_start_byte: 18,
                    source_end_byte: 26,
                    source_start_line: 1,
                    source_start_column: 19,
                    source_end_line: 1,
                    source_end_column: 27,
                },
                old_target: "Beta".into(),
                new_target: "Archive/Beta".into(),
                new_text: "[[Archive/Beta]]".into(),
                status: LinkRepairStatus::Resolved,
                ambiguity: None,
            },
        ];
        let accepted: Vec<String> = batch.candidates.iter().map(|c| c.id.clone()).collect();
        let moves: Vec<(PathBuf, PathBuf)> = batch
            .moves
            .iter()
            .map(|change| (change.from.clone(), change.to.clone()))
            .collect();
        let store = NativeWorkspaceStore::new(dir.path());
        let tx = build_batch_link_repair_transaction(
            &store,
            &moves,
            &batch.candidates,
            &accepted,
            batch_link_repair_transaction_summary(moves.len(), accepted.len()),
        )
        .unwrap();
        apply_transaction(&dir.path().to_string_lossy(), tx).unwrap();
        assert!(dir.path().join("Archive/Alpha.md").exists());
        assert!(dir.path().join("Archive/Beta.md").exists());
        let home = std::fs::read_to_string(dir.path().join("Notes/Home.md")).unwrap();
        assert_eq!(home, "See [[Archive/Alpha]] and [[Archive/Beta]].\n");
    }

    #[test]
    fn batch_summary_names_move_count() {
        assert_eq!(
            batch_link_repair_transaction_summary(3, 5),
            "Move 3 resources with 5 link repair(s)"
        );
    }

    #[test]
    fn path_change_serde_round_trip_shape() {
        let change = lattice_core::LinkRepairPathChange {
            from: PathBuf::from("Notes/A.md"),
            to: PathBuf::from("Archive/A.md"),
        };
        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["from"], "Notes/A.md");
        assert_eq!(json["to"], "Archive/A.md");
    }
}
