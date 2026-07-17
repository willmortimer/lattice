//! Tauri commands for reviewed link repair after resource renames.

use std::path::{Path, PathBuf};

use lattice_commands::{
    build_link_repair_transaction, dismiss_link_repair_proposal, link_repair_now,
    list_link_repair_proposals, load_link_repair_proposal, new_link_repair_plan_id,
    save_link_repair_proposal, Command as SemanticCommand, CommandEngine, Transaction,
};
use lattice_core::{
    LinkRepairPlan, LinkRepairProposalSummary, LinkRepairSource,
};
use lattice_index::WorkspaceIndex;
use lattice_storage::NativeWorkspaceStore;
use tauri::State;

use crate::commands::command_error_to_string;
use crate::resource_links::ResourceCatalogState;

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
    let tx = build_link_repair_transaction(
        &store,
        Some(SemanticCommand::ResourceRename {
            from: PathBuf::from(from),
            to: PathBuf::from(to),
        }),
        &plan,
        &accepted_candidate_ids,
        format!(
            "Rename {} to {} with {} link repair(s)",
            plan.rename_from.display(),
            plan.rename_to.display(),
            accepted_candidate_ids.len()
        ),
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

    #[test]
    fn external_proposal_persists_when_links_exist() {
        let dir = init_workspace();
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
        let summary = save_external_link_repair_proposal(
            dir.path(),
            Path::new("Notes/Other.md"),
            Path::new("Notes/Renamed.md"),
        )
        .unwrap()
        .expect("expected proposal");
        assert_eq!(summary.candidate_count, 1);
        let listed = list_link_repair_proposals(dir.path()).unwrap();
        assert_eq!(listed.len(), 1);
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
}
