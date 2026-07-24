//! Persist reviewable [`TransactionProposal`] bundles under `.lattice/proposals/`.
//!
//! This is the general agent/task proposal store (ADR 0018). Link-repair keeps
//! its own sibling directory and is not migrated here.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_core::OPERATIONAL_DIR;
use lattice_data::InterfaceDef;

use crate::artifact::ArtifactManifest;
use crate::command::{file_name, Command, Transaction};
use crate::contracts::{
    CommandPreview, CommandPreviewDetail, ProposalPreview, ProposalStatus, TransactionProposal,
    TransactionProposalSummary,
};
use crate::engine::CommandEngine;
use crate::workflow::WorkflowManifest;
use crate::{Error, Result};

/// Max UTF-8 chars retained in preview excerpts (bounded IPC payloads).
pub const PREVIEW_EXCERPT_CHARS: usize = 2_000;

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
///
/// When [`ProposalSource::idempotency_key`] is present (`{execution_id}:{step_id}`),
/// returns the existing **pending** proposal with that key instead of minting a
/// duplicate — safe for workflow step retries after a post-persist failure.
pub fn create_proposal(
    workspace_root: &Path,
    mut proposal: TransactionProposal,
) -> Result<TransactionProposal> {
    if let Some(existing) = find_pending_by_idempotency_key(workspace_root, &proposal)? {
        return Ok(existing);
    }
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

/// Look up a pending proposal whose source matches `{execution_id}:{step_id}`.
pub fn find_pending_by_idempotency_key(
    workspace_root: &Path,
    proposal: &TransactionProposal,
) -> Result<Option<TransactionProposal>> {
    let Some(key) = proposal.source.idempotency_key() else {
        return Ok(None);
    };
    let dir = proposals_dir(workspace_root);
    if !dir.is_dir() {
        return Ok(None);
    }
    for entry in fs::read_dir(&dir).map_err(|source| Error::io(&dir, source))? {
        let entry = entry.map_err(|source| Error::io(&dir, source))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let payload = fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
        let existing: TransactionProposal = serde_json::from_str(&payload)?;
        if existing.status != ProposalStatus::Pending {
            continue;
        }
        if existing.source.idempotency_key().as_deref() == Some(key.as_str()) {
            return Ok(Some(existing));
        }
    }
    Ok(None)
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

/// Validate that a selected subset can apply against the current workspace.
///
/// Runs [`CommandEngine`] precondition checks. When validation fails with
/// [`Error::NotFound`], looks for an earlier unselected command that would
/// create the missing path and returns [`Error::SubsetMissingDependency`].
pub fn validate_proposal_subset(
    workspace_root: &Path,
    proposal: &TransactionProposal,
    selected_indices: &[usize],
) -> Result<()> {
    let tx = build_proposal_transaction(proposal, selected_indices)?;
    let engine = CommandEngine::open(workspace_root)?;
    match engine.validate(&tx.commands) {
        Ok(()) => Ok(()),
        Err(Error::NotFound { path }) => {
            if let Some(required_index) =
                find_missing_predecessor(proposal, selected_indices, &path)
            {
                return Err(Error::SubsetMissingDependency {
                    required_index,
                    path,
                });
            }
            Err(Error::NotFound { path })
        }
        Err(other) => Err(other),
    }
}

/// Build a workspace-aware preview for every command, validating `selected_indices`.
///
/// Command detail rows always cover the full proposal. An empty selection is
/// reported as invalid (Accept requires at least one command).
pub fn preview_proposal(
    workspace_root: &Path,
    proposal: &TransactionProposal,
    selected_indices: &[usize],
) -> Result<ProposalPreview> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for &index in selected_indices {
        if index >= proposal.commands.len() {
            return Err(Error::InvalidResourceTarget {
                path: PathBuf::from(".lattice/proposals"),
                reason: format!(
                    "command index {index} is out of range (0..{})",
                    proposal.commands.len()
                ),
            });
        }
        if seen.insert(index) {
            selected.push(index);
        }
    }

    let engine = CommandEngine::open(workspace_root)?;
    let mut commands = Vec::with_capacity(proposal.commands.len());
    for (index, command) in proposal.commands.iter().enumerate() {
        commands.push(preview_command(&engine, workspace_root, index, command)?);
    }

    let mut subset_errors = Vec::new();
    let mut missing_predecessors = Vec::new();
    let subset_valid = if selected.is_empty() {
        subset_errors.push("accept requires at least one command index".into());
        false
    } else {
        match validate_proposal_subset(workspace_root, proposal, &selected) {
            Ok(()) => true,
            Err(err) => {
                if let Error::SubsetMissingDependency { required_index, .. } = &err {
                    missing_predecessors.push(*required_index);
                }
                subset_errors.push(err.to_string());
                false
            }
        }
    };

    Ok(ProposalPreview {
        proposal_id: proposal.id.clone(),
        commands,
        subset_valid,
        subset_errors,
        missing_predecessors,
    })
}

/// Apply selected commands through [`CommandEngine`], then remove the proposal.
pub fn apply_proposal(workspace_root: &Path, id: &str, selected_indices: &[usize]) -> Result<()> {
    let proposal = load_proposal(workspace_root, id)?;
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::InvalidResourceTarget {
            path: PathBuf::from(".lattice/proposals").join(format!("{id}.json")),
            reason: format!("proposal is not pending (status={:?})", proposal.status),
        });
    }
    validate_proposal_subset(workspace_root, &proposal, selected_indices)?;
    let tx = build_proposal_transaction(&proposal, selected_indices)?;
    let mut engine = CommandEngine::open(workspace_root)?;
    engine.apply(tx)?;
    dismiss_proposal(workspace_root, id)?;
    Ok(())
}

fn path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn truncate_excerpt(text: &str) -> (String, bool) {
    let mut truncated = false;
    let mut excerpt = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= PREVIEW_EXCERPT_CHARS {
            truncated = true;
            break;
        }
        excerpt.push(ch);
    }
    if truncated {
        excerpt.push('…');
    }
    (excerpt, truncated)
}

fn utf8_from_bytes(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

fn command_type_tag(command: &Command) -> String {
    match serde_json::to_value(command) {
        Ok(serde_json::Value::Object(map)) => map
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("command")
            .to_string(),
        _ => "command".into(),
    }
}

fn field_summary(values: &std::collections::BTreeMap<String, lattice_data::CellValue>) -> String {
    if values.is_empty() {
        return "(no fields)".into();
    }
    let keys: Vec<&str> = values.keys().map(String::as_str).collect();
    if keys.len() <= 6 {
        keys.join(", ")
    } else {
        format!("{} fields ({}, …)", keys.len(), keys[..4].join(", "))
    }
}

/// Paths a create-style (or rename/move destination) command would introduce.
fn paths_created_by(command: &Command) -> Vec<PathBuf> {
    match command {
        Command::PageCreate { path, .. }
        | Command::ResourceCreate { path, .. }
        | Command::FolderCreate { path }
        | Command::TableCreate { path, .. }
        | Command::DatasetCreate { path, .. } => vec![path.clone()],
        Command::ResourceRename { to, .. } => vec![to.clone()],
        Command::ResourceMove { from, to_dir } => vec![to_dir.join(file_name(from))],
        _ => Vec::new(),
    }
}

fn find_missing_predecessor(
    proposal: &TransactionProposal,
    selected_indices: &[usize],
    missing_path: &Path,
) -> Option<usize> {
    let selected: BTreeSet<usize> = selected_indices.iter().copied().collect();
    let min_selected = selected.iter().copied().min()?;
    for index in 0..min_selected {
        if selected.contains(&index) {
            continue;
        }
        let Some(command) = proposal.commands.get(index) else {
            continue;
        };
        if paths_created_by(command)
            .iter()
            .any(|created| created == missing_path)
        {
            return Some(index);
        }
    }
    None
}

fn preview_text_resource(
    path: &Path,
    content: &str,
    kind_hint: &str,
) -> (String, Option<CommandPreviewDetail>) {
    let display = path_display(path);
    let (excerpt, truncated) = truncate_excerpt(content);
    let lower = display.to_ascii_lowercase();
    if lower.ends_with(".workflow.yaml") {
        let parsed = WorkflowManifest::parse(path, content).ok();
        let summary = match &parsed {
            Some(manifest) => format!(
                "Create workflow {} ({})",
                manifest.name,
                if manifest.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
            None => format!("Create workflow {display}"),
        };
        return (
            summary,
            Some(CommandPreviewDetail::WorkflowSummary {
                path: display,
                name: parsed.as_ref().map(|m| m.name.clone()),
                step_count: parsed.as_ref().map(|m| m.steps.len()),
                excerpt,
                truncated,
            }),
        );
    }
    if lower.ends_with(".interface.yaml") {
        let parsed = InterfaceDef::parse_str(content, path).ok();
        let summary = match &parsed {
            Some(iface) => format!(
                "Create interface {} ({} component{})",
                iface.title.as_deref().unwrap_or(iface.name.as_str()),
                iface.components.len(),
                if iface.components.len() == 1 { "" } else { "s" }
            ),
            None => format!("Create interface {display}"),
        };
        return (
            summary,
            Some(CommandPreviewDetail::InterfaceSummary {
                path: display,
                name: parsed.as_ref().map(|m| m.name.clone()),
                title: parsed.as_ref().and_then(|m| m.title.clone()),
                component_count: parsed.as_ref().map(|m| m.components.len()),
                excerpt,
                truncated,
            }),
        );
    }
    if lower.ends_with("artifact.yaml") {
        let parsed = ArtifactManifest::parse_str(content, path).ok();
        let summary = match &parsed {
            Some(manifest) => format!(
                "Create artifact {} → {}",
                manifest.title.as_deref().unwrap_or("untitled"),
                manifest.entrypoint
            ),
            None => format!("Create artifact manifest {display}"),
        };
        return (
            summary,
            Some(CommandPreviewDetail::ArtifactSummary {
                path: display,
                title: parsed.as_ref().and_then(|m| m.title.clone()),
                entrypoint: parsed.as_ref().map(|m| m.entrypoint.clone()),
                excerpt,
                truncated,
            }),
        );
    }
    (
        format!("{kind_hint} {display}"),
        Some(CommandPreviewDetail::TextCreate {
            path: display,
            content_excerpt: excerpt,
            truncated,
            byte_len: content.len(),
        }),
    )
}

fn preview_command(
    engine: &CommandEngine,
    workspace_root: &Path,
    index: usize,
    command: &Command,
) -> Result<CommandPreview> {
    let command_type = command_type_tag(command);
    let touched_paths: Vec<String> = command
        .touched_paths()
        .into_iter()
        .map(|path| path_display(&path))
        .collect();

    let mut warnings = Vec::new();
    if let Err(err) = engine.validate_one(command) {
        // Surface precondition issues as review warnings (AlreadyExists, NotFound, …).
        warnings.push(err.to_string());
    }

    let (summary, detail) = match command {
        Command::PageCreate { path, content } => {
            preview_text_resource(path, content, "Create page")
        }
        Command::ResourceCreate { path, content } => {
            if let Some(text) = utf8_from_bytes(content) {
                preview_text_resource(path, &text, "Create resource")
            } else {
                let display = path_display(path);
                (
                    format!("Create binary resource {display} ({} bytes)", content.len()),
                    Some(CommandPreviewDetail::FileOp {
                        operation: "resource-create".into(),
                        paths: vec![display],
                        metadata: BTreeMap::from([("bytes".into(), content.len().to_string())]),
                    }),
                )
            }
        }
        Command::PageUpdate { path, content, .. } => {
            let display = path_display(path);
            let (after_excerpt, after_truncated) = truncate_excerpt(content);
            let before_excerpt = fs::read(workspace_root.join(path))
                .ok()
                .and_then(|bytes| utf8_from_bytes(&bytes))
                .map(|text| truncate_excerpt(&text).0);
            let truncated = after_truncated;
            (
                format!("Update page {display}"),
                Some(CommandPreviewDetail::TextDiff {
                    path: display,
                    before_excerpt,
                    after_excerpt,
                    truncated,
                }),
            )
        }
        Command::ResourceUpdate { path, content, .. } => {
            let display = path_display(path);
            if let Some(text) = utf8_from_bytes(content) {
                let (after_excerpt, truncated) = truncate_excerpt(&text);
                let before_excerpt = fs::read(workspace_root.join(path))
                    .ok()
                    .and_then(|bytes| utf8_from_bytes(&bytes))
                    .map(|text| truncate_excerpt(&text).0);
                (
                    format!("Update resource {display}"),
                    Some(CommandPreviewDetail::TextDiff {
                        path: display,
                        before_excerpt,
                        after_excerpt,
                        truncated,
                    }),
                )
            } else {
                (
                    format!("Update binary resource {display} ({} bytes)", content.len()),
                    Some(CommandPreviewDetail::FileOp {
                        operation: "resource-update".into(),
                        paths: vec![display],
                        metadata: BTreeMap::from([("bytes".into(), content.len().to_string())]),
                    }),
                )
            }
        }
        Command::WorkspaceManifestUpdate { .. } => (
            "Update workspace manifest (lattice.yaml)".into(),
            Some(CommandPreviewDetail::FileOp {
                operation: "workspace-manifest-update".into(),
                paths: vec!["lattice.yaml".into()],
                metadata: BTreeMap::new(),
            }),
        ),
        Command::ResourceRename { from, to } => (
            format!("Rename {} → {}", path_display(from), path_display(to)),
            Some(CommandPreviewDetail::FileOp {
                operation: "resource-rename".into(),
                paths: vec![path_display(from), path_display(to)],
                metadata: BTreeMap::new(),
            }),
        ),
        Command::ResourceMove { from, to_dir } => {
            let dest = to_dir.join(file_name(from));
            (
                format!("Move {} → {}", path_display(from), path_display(&dest)),
                Some(CommandPreviewDetail::FileOp {
                    operation: "resource-move".into(),
                    paths: vec![path_display(from), path_display(&dest)],
                    metadata: BTreeMap::from([("toDir".into(), path_display(to_dir))]),
                }),
            )
        }
        Command::ResourceDelete { path } => (
            format!("Delete {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "resource-delete".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::new(),
            }),
        ),
        Command::FolderCreate { path } => (
            format!("Create folder {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "folder-create".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::new(),
            }),
        ),
        Command::TableCreate {
            path,
            title,
            table_name,
        } => (
            format!("Create table package {} ({table_name})", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "table-create".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([
                    ("title".into(), title.clone()),
                    ("tableName".into(), table_name.clone()),
                ]),
            }),
        ),
        Command::DatasetCreate {
            path,
            title,
            description,
        } => {
            let mut metadata = BTreeMap::from([("title".into(), title.clone())]);
            if let Some(desc) = description {
                metadata.insert("description".into(), desc.clone());
            }
            (
                format!("Create dataset {}", path_display(path)),
                Some(CommandPreviewDetail::FileOp {
                    operation: "dataset-create".into(),
                    paths: vec![path_display(path)],
                    metadata,
                }),
            )
        }
        Command::TableAdd {
            path, table_name, ..
        } => (
            format!("Add table {table_name} to {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "table-add".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("tableName".into(), table_name.clone())]),
            }),
        ),
        Command::TableDrop {
            path, table_name, ..
        } => (
            format!("Drop table {table_name} from {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "table-drop".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("tableName".into(), table_name.clone())]),
            }),
        ),
        Command::ColumnsAdd {
            path,
            table,
            columns,
            ..
        } => (
            format!(
                "Add {} column(s) to {table} in {}",
                columns.len(),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "columns-add".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([
                    ("table".into(), table.clone()),
                    ("columnCount".into(), columns.len().to_string()),
                ]),
            }),
        ),
        Command::ColumnsRemove {
            path,
            table,
            columns,
            ..
        } => (
            format!(
                "Remove {} column(s) from {table} in {}",
                columns.len(),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "columns-remove".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([
                    ("table".into(), table.clone()),
                    ("columnCount".into(), columns.len().to_string()),
                ]),
            }),
        ),
        Command::RecordInsert {
            path,
            table,
            values,
            id,
        } => (
            format!("Insert record into {table} ({})", path_display(path)),
            Some(CommandPreviewDetail::RecordChange {
                path: path_display(path),
                table: table.clone(),
                operation: "insert".into(),
                id: id.clone(),
                field_summary: field_summary(values),
            }),
        ),
        Command::RecordUpdate {
            path,
            table,
            id,
            values,
            ..
        } => (
            format!("Update record {id} in {table} ({})", path_display(path)),
            Some(CommandPreviewDetail::RecordChange {
                path: path_display(path),
                table: table.clone(),
                operation: "update".into(),
                id: Some(id.clone()),
                field_summary: field_summary(values),
            }),
        ),
        Command::RecordDelete {
            path, table, id, ..
        } => (
            format!("Delete record {id} from {table} ({})", path_display(path)),
            Some(CommandPreviewDetail::RecordChange {
                path: path_display(path),
                table: table.clone(),
                operation: "delete".into(),
                id: Some(id.clone()),
                field_summary: String::new(),
            }),
        ),
        Command::ViewSave {
            path, view_name, ..
        } => (
            format!("Save view {view_name} in {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "view-save".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("viewName".into(), view_name.clone())]),
            }),
        ),
        Command::FormSave {
            path, form_name, ..
        } => (
            format!("Save form {form_name} in {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "form-save".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("formName".into(), form_name.clone())]),
            }),
        ),
        Command::CanvasPlaceResource {
            path,
            resource_path,
            ..
        } => (
            format!(
                "Place {} on canvas {}",
                path_display(resource_path),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "canvas-place-resource".into(),
                paths: vec![path_display(path), path_display(resource_path)],
                metadata: BTreeMap::new(),
            }),
        ),
        Command::CanvasMoveNodes { path, nodes, .. } => (
            format!(
                "Move {} canvas node(s) on {}",
                nodes.len(),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "canvas-move-nodes".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("nodeCount".into(), nodes.len().to_string())]),
            }),
        ),
        Command::CanvasRemoveNodes { path, node_ids, .. } => (
            format!(
                "Remove {} canvas node(s) from {}",
                node_ids.len(),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "canvas-remove-nodes".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("nodeCount".into(), node_ids.len().to_string())]),
            }),
        ),
        Command::CanvasAddEdge { path, .. } => (
            format!("Add canvas edge on {}", path_display(path)),
            Some(CommandPreviewDetail::FileOp {
                operation: "canvas-add-edge".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::new(),
            }),
        ),
        Command::CanvasResizeNodes { path, nodes, .. } => (
            format!(
                "Resize {} canvas node(s) on {}",
                nodes.len(),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "canvas-resize-nodes".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("nodeCount".into(), nodes.len().to_string())]),
            }),
        ),
        Command::CanvasRemoveEdges { path, edge_ids, .. } => (
            format!(
                "Remove {} canvas edge(s) from {}",
                edge_ids.len(),
                path_display(path)
            ),
            Some(CommandPreviewDetail::FileOp {
                operation: "canvas-remove-edges".into(),
                paths: vec![path_display(path)],
                metadata: BTreeMap::from([("edgeCount".into(), edge_ids.len().to_string())]),
            }),
        ),
        Command::CanvasAddTextNode { path, text, .. } => {
            let (excerpt, truncated) = truncate_excerpt(text);
            (
                format!("Add text node on {}", path_display(path)),
                Some(CommandPreviewDetail::TextCreate {
                    path: path_display(path),
                    content_excerpt: excerpt,
                    truncated,
                    byte_len: text.len(),
                }),
            )
        }
        Command::CanvasUpdateTextNode { path, text, .. } => {
            let (after_excerpt, truncated) = truncate_excerpt(text);
            (
                format!("Update text node on {}", path_display(path)),
                Some(CommandPreviewDetail::TextDiff {
                    path: path_display(path),
                    before_excerpt: None,
                    after_excerpt,
                    truncated,
                }),
            )
        }
    };

    Ok(CommandPreview {
        index,
        command_type,
        summary,
        touched_paths,
        warnings,
        detail,
    })
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
                execution_id: None,
                step_id: None,
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
                execution_id: None,
                step_id: None,
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
        assert!(!undone.transaction_id.is_empty());
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

    #[test]
    fn create_proposal_dedupes_by_execution_and_step() {
        let dir = workspace();
        let mut first = demo_proposal("", "Notes/A.md");
        first.source.execution_id = Some("exec-dedupe".into());
        first.source.step_id = Some("propose".into());
        let created = create_proposal(dir.path(), first).unwrap();

        // Simulate post-persist failure: caller retries create with same key.
        let mut retry = demo_proposal("", "Notes/A.md");
        retry.source.execution_id = Some("exec-dedupe".into());
        retry.source.step_id = Some("propose".into());
        retry.summary = "Different summary should not create a second proposal".into();
        let again = create_proposal(dir.path(), retry).unwrap();

        assert_eq!(again.id, created.id);
        assert_eq!(list_proposal_summaries(dir.path()).unwrap().len(), 1);
        let loaded = load_proposal(dir.path(), &created.id).unwrap();
        assert_eq!(loaded.summary, created.summary);
    }

    #[test]
    fn validate_subset_missing_create_before_update() {
        let dir = workspace();
        let proposal = TransactionProposal {
            id: "deps".into(),
            source: ProposalSource {
                source_type: ProposalSourceType::External,
                resource: None,
                execution_id: None,
                step_id: None,
            },
            summary: "Create then update".into(),
            commands: vec![
                Command::PageCreate {
                    path: PathBuf::from("Notes/New.md"),
                    content: "# new\n".into(),
                },
                Command::PageUpdate {
                    path: PathBuf::from("Notes/New.md"),
                    content: "# updated\n".into(),
                    base_revision: "sha256:deadbeef".into(),
                },
            ],
            affected_paths: vec!["Notes/New.md".into()],
            warnings: vec![],
            created_at: "2026-07-21T17:10:00Z".into(),
            status: ProposalStatus::Pending,
        };
        save_proposal(dir.path(), &proposal).unwrap();

        // Update alone cannot run: create predecessor was left unselected.
        let err = validate_proposal_subset(dir.path(), &proposal, &[1]).unwrap_err();
        match err {
            Error::SubsetMissingDependency {
                required_index,
                path,
            } => {
                assert_eq!(required_index, 0);
                assert_eq!(path, PathBuf::from("Notes/New.md"));
            }
            other => panic!("expected SubsetMissingDependency, got {other}"),
        }

        // Create alone is valid.
        validate_proposal_subset(dir.path(), &proposal, &[0]).unwrap();
    }

    #[test]
    fn validate_subset_path_collision() {
        let dir = workspace();
        let proposal = TransactionProposal {
            id: "collision".into(),
            source: ProposalSource {
                source_type: ProposalSourceType::External,
                resource: None,
                execution_id: None,
                step_id: None,
            },
            summary: "Two creates same path".into(),
            commands: vec![
                Command::PageCreate {
                    path: PathBuf::from("Notes/Same.md"),
                    content: "# one\n".into(),
                },
                Command::PageCreate {
                    path: PathBuf::from("Notes/Same.md"),
                    content: "# two\n".into(),
                },
            ],
            affected_paths: vec!["Notes/Same.md".into()],
            warnings: vec![],
            created_at: "2026-07-21T17:11:00Z".into(),
            status: ProposalStatus::Pending,
        };

        let err = validate_proposal_subset(dir.path(), &proposal, &[0, 1]).unwrap_err();
        assert!(matches!(err, Error::IntraTransactionConflict { .. }));
    }

    #[test]
    fn preview_surfaces_already_exists_warning() {
        let dir = workspace();
        let mut engine = CommandEngine::open(dir.path()).unwrap();
        engine
            .apply(Transaction::new(
                "seed",
                vec![Command::PageCreate {
                    path: PathBuf::from("Notes/Exists.md"),
                    content: "# already here\n".into(),
                }],
            ))
            .unwrap();

        let proposal =
            create_proposal(dir.path(), demo_proposal("exists", "Notes/Exists.md")).unwrap();

        let preview = preview_proposal(dir.path(), &proposal, &[0]).unwrap();
        assert!(!preview.subset_valid);
        assert_eq!(preview.commands.len(), 1);
        assert!(
            preview.commands[0]
                .warnings
                .iter()
                .any(|w| w.contains("already exists")),
            "warnings: {:?}",
            preview.commands[0].warnings
        );
        assert!(
            preview
                .subset_errors
                .iter()
                .any(|e| e.contains("already exists")),
            "subset_errors: {:?}",
            preview.subset_errors
        );
        match &preview.commands[0].detail {
            Some(CommandPreviewDetail::TextCreate { path, .. }) => {
                assert_eq!(path, "Notes/Exists.md");
            }
            other => panic!("expected TextCreate detail, got {other:?}"),
        }
    }

    #[test]
    fn preview_interface_resource_create_summary() {
        let dir = workspace();
        let yaml = r#"format: lattice-interface
version: 1
name: AgentDigest
title: Agent digest
views:
  - Main
components:
  - id: revenue
    type: metric
    span: 6
    title: Revenue
"#;
        let proposal = TransactionProposal {
            id: "iface".into(),
            source: ProposalSource {
                source_type: ProposalSourceType::Task,
                resource: Some("Tasks/AgentFirstLook.task".into()),
                execution_id: None,
                step_id: None,
            },
            summary: "Create interface".into(),
            commands: vec![Command::ResourceCreate {
                path: PathBuf::from("CRM.data/interfaces/AgentDigest.interface.yaml"),
                content: yaml.as_bytes().to_vec(),
            }],
            affected_paths: vec!["CRM.data/interfaces/AgentDigest.interface.yaml".into()],
            warnings: vec![],
            created_at: "2026-07-21T17:12:00Z".into(),
            status: ProposalStatus::Pending,
        };

        let preview = preview_proposal(dir.path(), &proposal, &[]).unwrap();
        assert_eq!(preview.commands.len(), 1);
        assert!(!preview.subset_valid);
        assert!(preview.commands[0].summary.contains("Agent digest"));
        // Full selection should preview as valid (path absent → create ok).
        let full = preview_proposal(dir.path(), &proposal, &[0]).unwrap();
        assert!(full.subset_valid);
        match &preview.commands[0].detail {
            Some(CommandPreviewDetail::InterfaceSummary {
                name,
                title,
                component_count,
                ..
            }) => {
                assert_eq!(name.as_deref(), Some("AgentDigest"));
                assert_eq!(title.as_deref(), Some("Agent digest"));
                assert_eq!(*component_count, Some(1));
            }
            other => panic!("expected InterfaceSummary, got {other:?}"),
        }
    }
}
