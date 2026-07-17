use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lattice_core::Workspace;
use lattice_data::{DataApp, Row};
use lattice_storage::{
    BufferedWriter, NativeWorkspaceStore, RecoveryJournal, ResourceMetadata, WorkspaceStore,
};

use crate::canvas::{self, CanvasEdit};
use crate::command::{
    file_name, view_file_path, Command, CommandOutcome, HistoryEntry, Transaction,
    TransactionReceipt, UndoReport,
};
use crate::history::{unix_now, unix_to_system, HistoryStore};
use crate::revisions::{
    ConflictEnvelope, HistoryCleanupReport, HistoryRetentionPolicy, ResourceRevisionDetail,
    ResourceRevisionSummary,
};
use crate::trash::{dispose, TrashPolicy};
use crate::{Error, Result, MAX_RESOURCE_EDIT_BYTES};

/// One applied command, carrying everything history needs to reverse it.
struct AppliedOp {
    forward: Command,
    inverse: Command,
    /// Bytes displaced by the command (previous page content, deleted file
    /// content). These, not the JSON, are authoritative for restoration.
    prior_content: Option<Vec<u8>>,
    /// Bytes materialized by the command, when this operation has a durable
    /// resource revision payload (create/update operations).
    after_content: Option<Vec<u8>>,
    /// Content hash of the command's target after it applied; `None` for
    /// deletes. Checked by the undo guard.
    resulting_revision: Option<String>,
}

struct RevisionCapture {
    path: PathBuf,
    parent_revision: Option<String>,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
}

impl AppliedOp {
    fn revision_capture(&self) -> Option<RevisionCapture> {
        match &self.forward {
            Command::PageCreate { path, content } => Some(RevisionCapture {
                path: path.clone(),
                parent_revision: None,
                before: self.prior_content.clone(),
                after: Some(content.as_bytes().to_vec()),
            }),
            Command::ResourceCreate { path, content } => Some(RevisionCapture {
                path: path.clone(),
                parent_revision: None,
                before: self.prior_content.clone(),
                after: Some(content.clone()),
            }),
            Command::PageUpdate {
                path,
                content,
                base_revision,
            } => Some(RevisionCapture {
                path: path.clone(),
                parent_revision: Some(base_revision.clone()),
                before: self.prior_content.clone(),
                after: Some(content.as_bytes().to_vec()),
            }),
            Command::ResourceUpdate {
                path,
                content,
                base_revision,
            } => Some(RevisionCapture {
                path: path.clone(),
                parent_revision: Some(base_revision.clone()),
                before: self.prior_content.clone(),
                after: Some(content.clone()),
            }),
            Command::WorkspaceManifestUpdate {
                content,
                base_revision,
            } => Some(RevisionCapture {
                path: PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME),
                parent_revision: Some(base_revision.clone()),
                before: self.prior_content.clone(),
                after: Some(content.as_bytes().to_vec()),
            }),
            Command::ResourceDelete { path } => Some(RevisionCapture {
                path: path.clone(),
                parent_revision: self.prior_content.as_deref().and_then(|bytes| {
                    lattice_storage::sha256_reader(std::io::Cursor::new(bytes)).ok()
                }),
                before: self.prior_content.clone(),
                after: None,
            }),
            Command::CanvasPlaceResource {
                path,
                base_revision,
                ..
            }
            | Command::CanvasMoveNodes {
                path,
                base_revision,
                ..
            }
            | Command::CanvasRemoveNodes {
                path,
                base_revision,
                ..
            } => Some(RevisionCapture {
                path: path.clone(),
                parent_revision: Some(base_revision.clone()),
                before: self.prior_content.clone(),
                after: self.after_content.clone(),
            }),
            _ => None,
        }
    }
}

/// The semantic command and transaction core (ADR 0007).
///
/// Every mutation in the product — GUI, CLI, future API/MCP — flows through
/// [`apply`](CommandEngine::apply). The engine:
///
/// 1. validates the preconditions of *all* commands in a transaction against
///    the current workspace state before mutating anything (fail fast);
/// 2. applies commands in order through `lattice-storage` — content writes go
///    through [`BufferedWriter`] so they are journaled before touching disk;
/// 3. records the transaction, its inverse operations, and any displaced
///    bytes in the durable history at `.lattice/history.sqlite`;
/// 4. serves [`undo`](CommandEngine::undo) / [`redo`](CommandEngine::redo)
///    from that history, refusing when a resource changed outside Lattice
///    since the transaction was recorded (ADR 0023).
///
/// v0 limitation: commands within one transaction must touch disjoint paths.
/// Sequential dependencies (create then update the same file in one tx) are
/// rejected as [`Error::IntraTransactionConflict`]; express them as separate
/// transactions instead.
pub struct CommandEngine {
    root: PathBuf,
    store: NativeWorkspaceStore,
    journal: RecoveryJournal,
    history: HistoryStore,
    trash_policy: TrashPolicy,
    session_id: String,
}

impl CommandEngine {
    /// Open the engine over an existing workspace root (must contain
    /// `lattice.yaml`).
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let workspace = Workspace::open(workspace_root)?;
        let root = workspace.root().to_path_buf();
        let store = NativeWorkspaceStore::new(&root);
        let journal = RecoveryJournal::open(&root)?;
        let history = HistoryStore::open(&root)?;
        Ok(CommandEngine {
            root,
            store,
            journal,
            history,
            trash_policy: TrashPolicy::default(),
            session_id: format!("commands-{}", uuid::Uuid::now_v7()),
        })
    }

    /// Change how deletes are disposed of (see [`TrashPolicy`]). Tests use
    /// [`TrashPolicy::LocalFallbackOnly`] to stay off the OS Trash.
    pub fn set_trash_policy(&mut self, policy: TrashPolicy) {
        self.trash_policy = policy;
    }

    /// Validate and apply a transaction atomically.
    ///
    /// If the transaction's idempotency key was already applied, returns the
    /// original receipt without mutating anything. If any command fails after
    /// earlier ones applied (rare: preconditions were validated up front),
    /// the earlier commands are rolled back via their inverses; a rollback
    /// failure is reported as [`Error::RollbackFailed`].
    ///
    /// A successful apply clears the redo stack.
    pub fn apply(&mut self, tx: Transaction) -> Result<TransactionReceipt> {
        if let Some(key) = &tx.idempotency_key {
            if let Some(stored) = self.history.find_by_idempotency_key(key)? {
                let ops = self.history.operations(stored.rowid)?;
                return Ok(TransactionReceipt {
                    transaction_id: stored.tx_id,
                    summary: stored.summary,
                    outcomes: ops
                        .into_iter()
                        .map(|o| CommandOutcome {
                            resulting_revision: o.resulting_revision,
                        })
                        .collect(),
                    idempotent_replay: true,
                });
            }
        }

        self.validate(&tx.commands)?;

        let mut applied: Vec<AppliedOp> = Vec::with_capacity(tx.commands.len());
        for (index, command) in tx.commands.iter().enumerate() {
            match self.apply_one(command) {
                Ok(op) => applied.push(op),
                Err(source) => {
                    if let Err(rollback) = self.rollback(&applied) {
                        return Err(Error::RollbackFailed {
                            index,
                            source: Box::new(source),
                            rollback_error: rollback.to_string(),
                        });
                    }
                    return Err(source);
                }
            }
        }

        self.history.clear_redo_stack()?;
        let created_at = unix_now();
        let rowid = self.history.insert_transaction(
            &tx.id,
            &tx.summary,
            created_at,
            tx.idempotency_key.as_deref(),
        )?;
        for (seq, op) in applied.iter().enumerate() {
            self.history.insert_operation(
                rowid,
                seq as i64,
                &serde_json::to_string(&op.forward)?,
                &serde_json::to_string(&op.inverse)?,
                op.prior_content.as_deref(),
                op.after_content.as_deref(),
                op.resulting_revision.as_deref(),
            )?;
            if let Some(capture) = op.revision_capture() {
                self.history.revisions().record_local_revision(
                    &format!("{}:{seq}", tx.id),
                    &tx.id,
                    &tx.summary,
                    seq,
                    &capture.path,
                    capture.parent_revision.as_deref(),
                    capture.before.as_deref(),
                    capture.after.as_deref(),
                )?;
            }
        }

        Ok(TransactionReceipt {
            transaction_id: tx.id,
            summary: tx.summary,
            outcomes: applied
                .into_iter()
                .map(|op| CommandOutcome {
                    resulting_revision: op.resulting_revision,
                })
                .collect(),
            idempotent_replay: false,
        })
    }

    /// Undo the most recent not-undone transaction. Returns `Ok(None)` when
    /// there is nothing to undo.
    ///
    /// Before touching anything, every affected resource is checked against
    /// the revision the transaction recorded; if any was modified outside
    /// Lattice since, the undo is refused with [`Error::RevisionGuard`]
    /// (ADR 0023). Directory deletes cannot be undone from history — their
    /// bytes live in the Trash — and refuse with
    /// [`Error::UndoDirectoryDelete`].
    pub fn undo(&mut self) -> Result<Option<UndoReport>> {
        let Some(stored) = self.history.find_active_latest()? else {
            return Ok(None);
        };
        let ops = self.history.operations(stored.rowid)?;
        let parsed = ops
            .iter()
            .map(|o| -> Result<(Command, Command)> {
                Ok((
                    serde_json::from_str(&o.forward_json)?,
                    serde_json::from_str(&o.inverse_json)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        // Guard phase: the workspace must be exactly as the transaction left
        // it, for every operation, before we mutate anything.
        for ((forward, _), op) in parsed.iter().zip(&ops) {
            self.guard_undo(forward, op.prior_content.as_deref(), &op.resulting_revision)?;
        }

        // Apply inverses newest-first.
        for ((_, inverse), op) in parsed.iter().zip(&ops).rev() {
            self.apply_inverse(inverse, op.prior_content.as_deref())?;
        }

        self.history.set_undone(stored.rowid, true)?;
        Ok(Some(UndoReport {
            transaction_id: stored.tx_id,
            summary: stored.summary,
        }))
    }

    /// Redo the most recently undone transaction. Returns `Ok(None)` when the
    /// redo stack is empty.
    ///
    /// The forward commands are re-validated against the current state before
    /// re-applying. Because undo restored the pre-transaction state, these
    /// preconditions double as the external-edit guard: any modification since
    /// the undo makes them fail.
    pub fn redo(&mut self) -> Result<Option<UndoReport>> {
        let Some(stored) = self.history.find_undone_earliest()? else {
            return Ok(None);
        };
        let ops = self.history.operations(stored.rowid)?;
        let forwards = ops
            .iter()
            .map(|o| Ok(serde_json::from_str(&o.forward_json)?))
            .collect::<Result<Vec<Command>>>()?;

        self.validate(&forwards)?;

        let mut applied: Vec<AppliedOp> = Vec::with_capacity(forwards.len());
        for (index, command) in forwards.iter().enumerate() {
            match self.apply_one(command) {
                Ok(op) => applied.push(op),
                Err(source) => {
                    if let Err(rollback) = self.rollback(&applied) {
                        return Err(Error::RollbackFailed {
                            index,
                            source: Box::new(source),
                            rollback_error: rollback.to_string(),
                        });
                    }
                    return Err(source);
                }
            }
        }

        // Re-applying may have displaced different bytes (e.g. a delete whose
        // file was restored by undo); refresh the recorded outcomes.
        for (seq, op) in applied.iter().enumerate() {
            self.history.update_operation_outcome(
                stored.rowid,
                seq as i64,
                op.prior_content.as_deref(),
                op.after_content.as_deref(),
                op.resulting_revision.as_deref(),
            )?;
        }
        self.history.set_undone(stored.rowid, false)?;
        Ok(Some(UndoReport {
            transaction_id: stored.tx_id,
            summary: stored.summary,
        }))
    }

    /// The most recent `limit` transactions, newest first.
    pub fn history(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        Ok(self
            .history
            .list(limit)?
            .into_iter()
            .map(|(t, command_count)| HistoryEntry {
                id: t.tx_id,
                summary: t.summary,
                created_at: unix_to_system(t.created_at),
                idempotency_key: t.idempotency_key,
                undone: t.undone,
                command_count,
            })
            .collect())
    }

    /// List durable revisions for one workspace-relative resource.
    pub fn list_resource_revisions(
        &self,
        path: &Path,
        limit: usize,
    ) -> Result<Vec<ResourceRevisionSummary>> {
        self.history
            .revisions()
            .list_resource_revisions(path, limit)
    }

    /// Load the base/local/incoming payload metadata and diff for one revision.
    pub fn resource_revision_detail(
        &self,
        path: &Path,
        revision_id: &str,
    ) -> Result<Option<ResourceRevisionDetail>> {
        self.history.revisions().get_detail(path, revision_id)
    }

    /// Record a file-system revision observed outside the command engine.
    /// Missing prior bytes remain missing in the resulting envelope.
    pub fn record_external_revision(
        &self,
        path: &Path,
        prior: Option<&[u8]>,
        after: &[u8],
        conflict: Option<&ConflictEnvelope>,
        incoming: Option<&[u8]>,
    ) -> Result<ResourceRevisionSummary> {
        self.history
            .revisions()
            .record_external_revision(path, prior, after, conflict, incoming)
    }

    /// Run explicit/idle-callable history payload cleanup. Transaction and
    /// revision metadata are retained even when payloads are reclaimed.
    pub fn cleanup_history(
        &self,
        policy: HistoryRetentionPolicy,
        dry_run: bool,
    ) -> Result<HistoryCleanupReport> {
        self.history.revisions().cleanup(policy, dry_run)
    }

    /// Revert a resource to a recorded local state by applying a fresh,
    /// guarded semantic transaction. The caller must provide the revision it
    /// currently sees; a mismatch refuses the revert without mutation.
    pub fn revert_resource_revision(
        &mut self,
        path: &Path,
        revision_id: &str,
        expected_current_revision: &str,
    ) -> Result<TransactionReceipt> {
        let detail = self
            .resource_revision_detail(path, revision_id)?
            .ok_or_else(|| Error::RevisionNotFound {
                path: path.to_path_buf(),
                revision: revision_id.to_string(),
            })?;
        let current = self.metadata_opt(path)?.map(|meta| meta.revision.hash);
        let found = current.clone().unwrap_or_else(|| "(absent)".into());
        if found != expected_current_revision {
            return Err(Error::StaleBaseRevision {
                path: path.to_path_buf(),
                expected: expected_current_revision.to_string(),
                found,
            });
        }

        let command = match detail.local {
            Some(payload) => {
                let bytes = payload
                    .bytes
                    .ok_or_else(|| Error::RevisionPayloadUnavailable {
                        path: path.to_path_buf(),
                    })?;
                match String::from_utf8(bytes.clone()) {
                    Ok(content) if current.is_some() => Command::PageUpdate {
                        path: path.to_path_buf(),
                        content,
                        base_revision: expected_current_revision.to_string(),
                    },
                    Ok(content) => Command::PageCreate {
                        path: path.to_path_buf(),
                        content,
                    },
                    Err(_) if current.is_some() => Command::ResourceUpdate {
                        path: path.to_path_buf(),
                        content: bytes,
                        base_revision: expected_current_revision.to_string(),
                    },
                    Err(_) => Command::ResourceCreate {
                        path: path.to_path_buf(),
                        content: bytes,
                    },
                }
            }
            None if current.is_some() => Command::ResourceDelete {
                path: path.to_path_buf(),
            },
            None => {
                return Err(Error::NotFound {
                    path: path.to_path_buf(),
                })
            }
        };
        self.apply(Transaction::new(
            format!("Revert {} to {revision_id}", path.display()),
            vec![command],
        ))
    }

    // ---------------------------------------------------------------------
    // Validation
    // ---------------------------------------------------------------------

    /// Validate every command against the *current* workspace state, without
    /// mutating anything. Also rejects transactions where two commands touch
    /// the same path (v0: no sequential dependencies within a transaction).
    fn validate(&self, commands: &[Command]) -> Result<()> {
        let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
        for command in commands {
            for path in command.touched_paths() {
                if !seen.insert(path.clone()) {
                    return Err(Error::IntraTransactionConflict { path });
                }
            }
        }
        for command in commands {
            self.validate_one(command)?;
        }
        Ok(())
    }

    fn validate_one(&self, command: &Command) -> Result<()> {
        match command {
            Command::PageCreate { path, .. } | Command::ResourceCreate { path, .. } => {
                match self.metadata_opt(path)? {
                    None => Ok(()),
                    Some(_) => Err(Error::AlreadyExists { path: path.clone() }),
                }
            }
            Command::PageUpdate {
                path,
                base_revision,
                content,
            } => {
                self.validate_edit_size(path, content.len())?;
                let meta = self
                    .metadata_opt(path)?
                    .ok_or_else(|| Error::NotFound { path: path.clone() })?;
                if meta.revision.hash != *base_revision {
                    return Err(Error::StaleBaseRevision {
                        path: path.clone(),
                        expected: base_revision.clone(),
                        found: meta.revision.hash,
                    });
                }
                Ok(())
            }
            Command::ResourceUpdate {
                path,
                content,
                base_revision,
            } => {
                self.validate_edit_size(path, content.len())?;
                self.validate_resource_update_target(path)?;
                let meta = self
                    .metadata_opt(path)?
                    .ok_or_else(|| Error::NotFound { path: path.clone() })?;
                if meta.is_dir {
                    return Err(Error::NotFound { path: path.clone() });
                }
                if meta.revision.len > MAX_RESOURCE_EDIT_BYTES as u64 {
                    return Err(Error::EditTooLarge {
                        path: path.clone(),
                        size: meta.revision.len,
                        max: MAX_RESOURCE_EDIT_BYTES as u64,
                    });
                }
                if meta.revision.hash != *base_revision {
                    return Err(Error::StaleBaseRevision {
                        path: path.clone(),
                        expected: base_revision.clone(),
                        found: meta.revision.hash,
                    });
                }
                Ok(())
            }
            Command::WorkspaceManifestUpdate {
                content,
                base_revision,
            } => {
                let path = PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME);
                lattice_core::WorkspaceManifest::parse(&path, content)?;
                let meta = self
                    .metadata_opt(&path)?
                    .ok_or_else(|| Error::NotFound { path: path.clone() })?;
                if meta.revision.hash != *base_revision {
                    return Err(Error::StaleBaseRevision {
                        path,
                        expected: base_revision.clone(),
                        found: meta.revision.hash,
                    });
                }
                Ok(())
            }
            Command::ResourceRename { from, to } => {
                if self.metadata_opt(from)?.is_none() {
                    return Err(Error::NotFound { path: from.clone() });
                }
                if self.metadata_opt(to)?.is_some() {
                    return Err(Error::AlreadyExists { path: to.clone() });
                }
                Ok(())
            }
            Command::ResourceMove { from, to_dir } => {
                if self.metadata_opt(from)?.is_none() {
                    return Err(Error::NotFound { path: from.clone() });
                }
                let dir = self.metadata_opt(to_dir)?.ok_or_else(|| Error::NotFound {
                    path: to_dir.clone(),
                })?;
                if !dir.is_dir {
                    return Err(Error::NotADirectory {
                        path: to_dir.clone(),
                    });
                }
                let dest = to_dir.join(file_name(from));
                if self.metadata_opt(&dest)?.is_some() {
                    return Err(Error::AlreadyExists { path: dest });
                }
                Ok(())
            }
            Command::ResourceDelete { path } => {
                if self.metadata_opt(path)?.is_none() {
                    return Err(Error::NotFound { path: path.clone() });
                }
                Ok(())
            }
            Command::TableCreate { path, .. } => match self.metadata_opt(path)? {
                None => Ok(()),
                Some(_) => Err(Error::AlreadyExists { path: path.clone() }),
            },
            Command::RecordInsert { path, .. } => {
                if self.metadata_opt(path)?.is_none() {
                    return Err(Error::NotFound { path: path.clone() });
                }
                let app = self.open_data_app(path)?;
                if let Command::RecordInsert {
                    table,
                    id: Some(row_id),
                    ..
                } = command
                {
                    if app.get_row(table, row_id)?.is_some() {
                        return Err(Error::AlreadyExists { path: path.clone() });
                    }
                }
                Ok(())
            }
            Command::RecordUpdate {
                path,
                base_revision,
                table,
                id,
                ..
            } => {
                self.ensure_package_revision(path, base_revision)?;
                let app = self.open_data_app(path)?;
                if app.get_row(table, id)?.is_none() {
                    return Err(Error::NotFound { path: path.clone() });
                }
                Ok(())
            }
            Command::RecordDelete {
                path,
                base_revision,
                table,
                id,
                ..
            } => {
                self.ensure_package_revision(path, base_revision)?;
                let app = self.open_data_app(path)?;
                if app.get_row(table, id)?.is_none() {
                    return Err(Error::NotFound { path: path.clone() });
                }
                Ok(())
            }
            Command::ViewSave { path, .. } => {
                if self.metadata_opt(path)?.is_none() {
                    return Err(Error::NotFound { path: path.clone() });
                }
                Ok(())
            }
            Command::CanvasPlaceResource {
                path,
                base_revision,
                resource_path,
                node_id,
                x,
                y,
                width,
                height,
            } => {
                self.ensure_canvas_revision(path, base_revision)?;
                self.ensure_canvas_resource(resource_path)?;
                canvas::validate_edit(
                    path,
                    &self.store.read(path)?,
                    &CanvasEdit::Place {
                        resource_path: resource_path.clone(),
                        node_id: node_id.clone(),
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                    },
                )
            }
            Command::CanvasMoveNodes {
                path,
                base_revision,
                nodes,
            } => {
                self.ensure_canvas_revision(path, base_revision)?;
                canvas::validate_edit(
                    path,
                    &self.store.read(path)?,
                    &CanvasEdit::Move {
                        nodes: nodes.clone(),
                    },
                )
            }
            Command::CanvasRemoveNodes {
                path,
                base_revision,
                node_ids,
            } => {
                self.ensure_canvas_revision(path, base_revision)?;
                canvas::validate_edit(
                    path,
                    &self.store.read(path)?,
                    &CanvasEdit::Remove {
                        node_ids: node_ids.clone(),
                    },
                )
            }
        }
    }

    // ---------------------------------------------------------------------
    // Forward application
    // ---------------------------------------------------------------------

    fn writer(&self) -> BufferedWriter<'_> {
        BufferedWriter::new(&self.store, &self.journal, self.session_id.clone())
    }

    fn validate_edit_size(&self, path: &Path, size: usize) -> Result<()> {
        if size > MAX_RESOURCE_EDIT_BYTES {
            return Err(Error::EditTooLarge {
                path: path.to_path_buf(),
                size: size as u64,
                max: MAX_RESOURCE_EDIT_BYTES as u64,
            });
        }
        Ok(())
    }

    fn validate_resource_update_target(&self, path: &Path) -> Result<()> {
        let components = path.components().collect::<Vec<_>>();
        let is_manifest = components.len() == 1
            && components.first().is_some_and(|component| {
                component.as_os_str() == lattice_core::WORKSPACE_MANIFEST_FILENAME
            });
        let is_operational = components.iter().any(|component| {
            component.as_os_str() == std::ffi::OsStr::new(lattice_core::OPERATIONAL_DIR)
        });
        if is_manifest || is_operational {
            return Err(Error::ResourceNotEditable {
                path: path.to_path_buf(),
                profile: "internal".into(),
            });
        }

        let inspection = lattice_core::inspect_resource(&self.root, path).map_err(|error| {
            Error::InvalidResourceTarget {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        if inspection.is_directory {
            return Err(Error::InvalidResourceTarget {
                path: path.to_path_buf(),
                reason: "directories are not file edit targets".into(),
            });
        }
        if !inspection.capabilities.can_update || inspection.encoding.is_none() {
            return Err(Error::ResourceNotEditable {
                path: path.to_path_buf(),
                profile: format!("{:?}", inspection.profile),
            });
        }
        Ok(())
    }

    fn apply_one(&self, command: &Command) -> Result<AppliedOp> {
        match command {
            Command::PageCreate { path, content } => {
                let revision = self.writer().write(path, content.as_bytes(), None)?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::ResourceDelete { path: path.clone() },
                    prior_content: None,
                    after_content: Some(content.as_bytes().to_vec()),
                    resulting_revision: Some(revision.hash),
                })
            }
            Command::ResourceCreate { path, content } => {
                let revision = self.writer().write(path, content, None)?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::ResourceDelete { path: path.clone() },
                    prior_content: None,
                    after_content: Some(content.clone()),
                    resulting_revision: Some(revision.hash),
                })
            }
            Command::PageUpdate {
                path,
                content,
                base_revision,
            } => {
                // Re-check at apply time so the revision handed to the writer
                // is the one the caller based the edit on, not whatever is on
                // disk now (the writer enforces it a final time).
                let meta = self.store.metadata(path)?;
                if meta.revision.hash != *base_revision {
                    return Err(Error::StaleBaseRevision {
                        path: path.clone(),
                        expected: base_revision.clone(),
                        found: meta.revision.hash,
                    });
                }
                let prior = self.store.read(path)?;
                let revision =
                    self.writer()
                        .write(path, content.as_bytes(), Some(&meta.revision))?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::PageUpdate {
                        path: path.clone(),
                        content: String::from_utf8_lossy(&prior).into_owned(),
                        base_revision: revision.hash.clone(),
                    },
                    prior_content: Some(prior),
                    after_content: Some(content.as_bytes().to_vec()),
                    resulting_revision: Some(revision.hash),
                })
            }
            Command::ResourceUpdate {
                path,
                content,
                base_revision,
            } => {
                self.validate_edit_size(path, content.len())?;
                self.validate_resource_update_target(path)?;
                let meta = self.store.metadata(path)?;
                if meta.is_dir {
                    return Err(Error::NotFound { path: path.clone() });
                }
                if meta.revision.len > MAX_RESOURCE_EDIT_BYTES as u64 {
                    return Err(Error::EditTooLarge {
                        path: path.clone(),
                        size: meta.revision.len,
                        max: MAX_RESOURCE_EDIT_BYTES as u64,
                    });
                }
                if meta.revision.hash != *base_revision {
                    return Err(Error::StaleBaseRevision {
                        path: path.clone(),
                        expected: base_revision.clone(),
                        found: meta.revision.hash,
                    });
                }
                let prior = self.store.read(path)?;
                let revision = self.writer().write(path, content, Some(&meta.revision))?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::ResourceUpdate {
                        path: path.clone(),
                        content: prior.clone(),
                        base_revision: revision.hash.clone(),
                    },
                    prior_content: Some(prior),
                    after_content: Some(content.clone()),
                    resulting_revision: Some(revision.hash),
                })
            }
            Command::WorkspaceManifestUpdate {
                content,
                base_revision,
            } => {
                let path = PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME);
                let meta = self.store.metadata(&path)?;
                if meta.revision.hash != *base_revision {
                    return Err(Error::StaleBaseRevision {
                        path,
                        expected: base_revision.clone(),
                        found: meta.revision.hash,
                    });
                }
                let prior = self.store.read(&path)?;
                let revision =
                    self.writer()
                        .write(&path, content.as_bytes(), Some(&meta.revision))?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::WorkspaceManifestUpdate {
                        content: String::from_utf8_lossy(&prior).into_owned(),
                        base_revision: revision.hash.clone(),
                    },
                    prior_content: Some(prior),
                    after_content: Some(content.as_bytes().to_vec()),
                    resulting_revision: Some(revision.hash),
                })
            }
            Command::ResourceRename { from, to } => {
                self.store.rename(from, to)?;
                let hash = self.store.metadata(to)?.revision.hash;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::ResourceRename {
                        from: to.clone(),
                        to: from.clone(),
                    },
                    prior_content: None,
                    after_content: None,
                    resulting_revision: Some(hash),
                })
            }
            Command::ResourceMove { from, to_dir } => {
                let dest = to_dir.join(file_name(from));
                self.store.rename(from, &dest)?;
                let hash = self.store.metadata(&dest)?.revision.hash;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::ResourceRename {
                        from: dest,
                        to: from.clone(),
                    },
                    prior_content: None,
                    after_content: None,
                    resulting_revision: Some(hash),
                })
            }
            Command::ResourceDelete { path } => {
                // `metadata` also normalizes and rejects escaping paths, so
                // the absolute join below is safe.
                let meta = self.store.metadata(path)?;
                let prior = if meta.is_dir {
                    None // directory bytes are only recoverable from the Trash
                } else {
                    Some(self.store.read(path)?)
                };
                dispose(&self.root, &self.root.join(path), self.trash_policy)?;
                let content = prior
                    .as_ref()
                    .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                    .unwrap_or_default();
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::PageCreate {
                        path: path.clone(),
                        content,
                    },
                    prior_content: prior,
                    after_content: None,
                    resulting_revision: None,
                })
            }
            Command::TableCreate {
                path,
                title,
                table_name,
            } => {
                let abs = self.root.join(path);
                let app = DataApp::create(&abs, title, table_name)?;
                let revision = app.package_revision()?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    // Undo removes the package directly (not via Trash) so redo
                    // can recreate a clean path without digging in Trash.
                    inverse: Command::ResourceDelete { path: path.clone() },
                    prior_content: None,
                    after_content: None,
                    resulting_revision: Some(revision),
                })
            }
            Command::RecordInsert {
                path,
                table,
                values,
                id,
            } => {
                let app = self.open_data_app(path)?;
                let row_id = if let Some(row_id) = id.clone() {
                    let mut row_values = values.clone();
                    row_values.insert(
                        "id".to_string(),
                        lattice_data::CellValue::Text(row_id.clone()),
                    );
                    let row = Row {
                        id: row_id.clone(),
                        values: row_values,
                    };
                    app.restore_row(table, &row)?;
                    row_id
                } else {
                    app.insert_row(table, values)?
                };
                let revision = app.package_revision()?;
                Ok(AppliedOp {
                    forward: Command::RecordInsert {
                        path: path.clone(),
                        table: table.clone(),
                        values: values.clone(),
                        id: Some(row_id.clone()),
                    },
                    inverse: Command::RecordDelete {
                        path: path.clone(),
                        table: table.clone(),
                        id: row_id,
                        base_revision: revision.clone(),
                    },
                    prior_content: None,
                    after_content: None,
                    resulting_revision: Some(revision),
                })
            }
            Command::RecordUpdate {
                path,
                table,
                id,
                values,
                base_revision,
            } => {
                self.ensure_package_revision(path, base_revision)?;
                let app = self.open_data_app(path)?;
                let prior_row = app
                    .get_row(table, id)?
                    .ok_or_else(|| Error::NotFound { path: path.clone() })?;
                let prior_values = row_values_without_id(&prior_row);
                app.update_row(table, id, values)?;
                let revision = app.package_revision()?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::RecordUpdate {
                        path: path.clone(),
                        table: table.clone(),
                        id: id.clone(),
                        values: prior_values,
                        base_revision: revision.clone(),
                    },
                    prior_content: Some(serde_json::to_vec(&prior_row)?),
                    after_content: None,
                    resulting_revision: Some(revision),
                })
            }
            Command::RecordDelete {
                path,
                table,
                id,
                base_revision,
            } => {
                self.ensure_package_revision(path, base_revision)?;
                let app = self.open_data_app(path)?;
                let prior_row = app
                    .get_row(table, id)?
                    .ok_or_else(|| Error::NotFound { path: path.clone() })?;
                app.delete_row(table, id)?;
                let revision = app.package_revision()?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::RecordInsert {
                        path: path.clone(),
                        table: table.clone(),
                        values: row_values_without_id(&prior_row),
                        id: Some(id.clone()),
                    },
                    prior_content: Some(serde_json::to_vec(&prior_row)?),
                    after_content: None,
                    resulting_revision: Some(revision),
                })
            }
            Command::ViewSave {
                path,
                view_name,
                content,
            } => {
                let view_path = view_file_path(path, view_name);
                let prior = self.read_view_opt(&view_path)?;
                if let Some(parent) = self.root.join(&view_path).parent() {
                    std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
                }
                let revision = self
                    .writer()
                    .write(&view_path, content.as_bytes(), None)?
                    .hash;
                let inverse = match prior {
                    Some(previous) => Command::ViewSave {
                        path: path.clone(),
                        view_name: view_name.clone(),
                        content: previous,
                    },
                    None => Command::ResourceDelete {
                        path: view_path.clone(),
                    },
                };
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse,
                    prior_content: None,
                    after_content: Some(content.as_bytes().to_vec()),
                    resulting_revision: Some(revision),
                })
            }
            Command::CanvasPlaceResource {
                path,
                base_revision,
                resource_path,
                node_id,
                x,
                y,
                width,
                height,
            } => self.apply_canvas_edit(
                command,
                path,
                base_revision,
                CanvasEdit::Place {
                    resource_path: resource_path.clone(),
                    node_id: node_id.clone(),
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                },
            ),
            Command::CanvasMoveNodes {
                path,
                base_revision,
                nodes,
            } => self.apply_canvas_edit(
                command,
                path,
                base_revision,
                CanvasEdit::Move {
                    nodes: nodes.clone(),
                },
            ),
            Command::CanvasRemoveNodes {
                path,
                base_revision,
                node_ids,
            } => self.apply_canvas_edit(
                command,
                path,
                base_revision,
                CanvasEdit::Remove {
                    node_ids: node_ids.clone(),
                },
            ),
        }
    }

    fn apply_canvas_edit(
        &self,
        command: &Command,
        path: &Path,
        base_revision: &str,
        edit: CanvasEdit,
    ) -> Result<AppliedOp> {
        self.ensure_canvas_revision(path, base_revision)?;
        let meta = self.store.metadata(path)?;
        let prior = self.store.read(path)?;
        let after = canvas::patch(path, &prior, &edit)?;
        self.validate_edit_size(path, after.len())?;
        let revision = self.writer().write(path, &after, Some(&meta.revision))?;
        Ok(AppliedOp {
            forward: command.clone(),
            inverse: Command::ResourceUpdate {
                path: path.to_path_buf(),
                content: prior.clone(),
                base_revision: revision.hash.clone(),
            },
            prior_content: Some(prior),
            after_content: Some(after),
            resulting_revision: Some(revision.hash),
        })
    }

    // ---------------------------------------------------------------------
    // Inverse application (undo, and rollback of a half-applied transaction)
    // ---------------------------------------------------------------------

    /// Roll back already-applied operations of a failed transaction, newest
    /// first. Nothing was recorded in history yet, so this is purely
    /// compensating.
    fn rollback(&self, applied: &[AppliedOp]) -> Result<()> {
        for op in applied.iter().rev() {
            self.apply_inverse(&op.inverse, op.prior_content.as_deref())?;
        }
        Ok(())
    }

    /// Execute one inverse command. `prior_content` (when present) is the
    /// authoritative byte content to restore; the JSON copy inside the
    /// command is a lossy rendering kept for auditability.
    fn apply_inverse(&self, inverse: &Command, prior_content: Option<&[u8]>) -> Result<()> {
        match inverse {
            // Inverse of PageCreate. A plain remove, not a trash trip: the
            // created content is preserved in history and restorable by redo.
            Command::ResourceDelete { path } => {
                self.store.remove(path)?;
                Ok(())
            }
            // Inverse of PageUpdate: restore the displaced bytes.
            Command::PageUpdate { path, content, .. } => {
                let bytes = prior_content
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| content.clone().into_bytes());
                let meta = self.store.metadata(path)?;
                self.writer().write(path, &bytes, Some(&meta.revision))?;
                Ok(())
            }
            Command::ResourceUpdate { path, content, .. } => {
                let bytes = prior_content
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| content.clone());
                let meta = self.store.metadata(path)?;
                self.writer().write(path, &bytes, Some(&meta.revision))?;
                Ok(())
            }
            Command::WorkspaceManifestUpdate { content, .. } => {
                let path = PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME);
                let bytes = prior_content
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| content.clone().into_bytes());
                let meta = self.store.metadata(&path)?;
                self.writer().write(&path, &bytes, Some(&meta.revision))?;
                Ok(())
            }
            // Inverse of ResourceDelete (file): re-materialize the bytes.
            Command::PageCreate { path, content } => {
                let bytes = prior_content
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| content.clone().into_bytes());
                self.writer().write(path, &bytes, None)?;
                Ok(())
            }
            Command::ResourceCreate { path, content } => {
                let bytes = prior_content
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| content.clone());
                self.writer().write(path, &bytes, None)?;
                Ok(())
            }
            // Inverse of ResourceRename / ResourceMove.
            Command::ResourceRename { from, to } => {
                self.store.rename(from, to)?;
                Ok(())
            }
            // Inverses are always expressed as renames, but stay exhaustive.
            Command::ResourceMove { from, to_dir } => {
                self.store.rename(from, &to_dir.join(file_name(from)))?;
                Ok(())
            }
            Command::TableCreate {
                path,
                title,
                table_name,
            } => {
                let abs = self.root.join(path);
                DataApp::create(&abs, title, table_name)?;
                Ok(())
            }
            Command::RecordInsert {
                path,
                table,
                values,
                id,
            } => {
                let app = self.open_data_app(path)?;
                if let Some(bytes) = prior_content {
                    let row: Row = serde_json::from_slice(bytes)?;
                    app.restore_row(table, &row)?;
                } else if let Some(row_id) = id {
                    let mut row_values = values.clone();
                    row_values.insert(
                        "id".to_string(),
                        lattice_data::CellValue::Text(row_id.clone()),
                    );
                    app.restore_row(
                        table,
                        &Row {
                            id: row_id.clone(),
                            values: row_values,
                        },
                    )?;
                } else {
                    app.insert_row(table, values)?;
                }
                Ok(())
            }
            Command::RecordUpdate {
                path,
                table,
                id,
                values,
                base_revision: _,
            } => {
                if let Some(bytes) = prior_content {
                    let row: Row = serde_json::from_slice(bytes)?;
                    self.open_data_app(path)?.update_row(
                        table,
                        id,
                        &row_values_without_id(&row),
                    )?;
                } else {
                    self.open_data_app(path)?.update_row(table, id, values)?;
                }
                Ok(())
            }
            Command::RecordDelete {
                path,
                table,
                id,
                base_revision: _,
            } => {
                self.open_data_app(path)?.delete_row(table, id)?;
                Ok(())
            }
            Command::ViewSave {
                path,
                view_name,
                content,
            } => {
                let view_path = view_file_path(path, view_name);
                if let Some(parent) = self.root.join(&view_path).parent() {
                    std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
                }
                let meta = self.store.metadata(&view_path).ok();
                self.writer().write(
                    &view_path,
                    content.as_bytes(),
                    meta.as_ref().map(|m| &m.revision),
                )?;
                Ok(())
            }
            Command::CanvasPlaceResource { .. }
            | Command::CanvasMoveNodes { .. }
            | Command::CanvasRemoveNodes { .. } => {
                unreachable!("canvas commands are never stored as inverse operations")
            }
        }
    }

    // ---------------------------------------------------------------------
    // Undo guard (ADR 0023)
    // ---------------------------------------------------------------------

    /// Verify one operation's target is exactly as the transaction left it.
    fn guard_undo(
        &self,
        forward: &Command,
        prior_content: Option<&[u8]>,
        resulting_revision: &Option<String>,
    ) -> Result<()> {
        match forward {
            Command::ResourceDelete { path } => {
                if prior_content.is_none() {
                    return Err(Error::UndoDirectoryDelete { path: path.clone() });
                }
                if let Some(found) = self.current_hash(path)? {
                    return Err(Error::RevisionGuard {
                        op: "undo",
                        path: path.clone(),
                        expected: "(absent)".into(),
                        found,
                    });
                }
                Ok(())
            }
            Command::ResourceRename { from, .. } | Command::ResourceMove { from, .. } => {
                // The destination must still carry the recorded revision…
                self.guard_hash(&forward.guard_path(), resulting_revision.as_deref())?;
                // …and the original location must still be vacant, or the
                // rename back would clobber whatever appeared there.
                if let Some(found) = self.current_hash(from)? {
                    return Err(Error::RevisionGuard {
                        op: "undo",
                        path: from.clone(),
                        expected: "(absent)".into(),
                        found,
                    });
                }
                Ok(())
            }
            Command::PageCreate { .. }
            | Command::ResourceCreate { .. }
            | Command::PageUpdate { .. }
            | Command::ResourceUpdate { .. }
            | Command::WorkspaceManifestUpdate { .. } => {
                self.guard_hash(&forward.guard_path(), resulting_revision.as_deref())
            }
            Command::TableCreate { path, .. } => {
                if self.metadata_opt(path)?.is_some() {
                    Ok(())
                } else {
                    Err(Error::RevisionGuard {
                        op: "undo",
                        path: path.clone(),
                        expected: "(package present)".into(),
                        found: "(absent)".into(),
                    })
                }
            }
            Command::RecordInsert {
                path,
                table,
                id: Some(row_id),
                ..
            } => self.guard_row_present(path, table, row_id),
            Command::RecordInsert { path, .. } => self.guard_package_revision(
                path,
                resulting_revision
                    .as_deref()
                    .ok_or_else(|| Error::RevisionGuard {
                        op: "undo",
                        path: path.clone(),
                        expected: "(recorded)".into(),
                        found: "(missing revision)".into(),
                    })?,
            ),
            Command::RecordUpdate {
                path,
                table,
                id,
                values,
                ..
            } => self.guard_row_values(path, table, id, values),
            Command::RecordDelete {
                path, table, id, ..
            } => self.guard_row_absent(path, table, id),
            Command::ViewSave { .. } => {
                self.guard_hash(&forward.guard_path(), resulting_revision.as_deref())
            }
            Command::CanvasPlaceResource { .. }
            | Command::CanvasMoveNodes { .. }
            | Command::CanvasRemoveNodes { .. } => {
                self.guard_hash(&forward.guard_path(), resulting_revision.as_deref())
            }
        }
    }

    fn guard_hash(&self, path: &Path, expected: Option<&str>) -> Result<()> {
        let expected = expected.unwrap_or("(unrecorded)");
        match self.current_hash(path)? {
            Some(found) if found == expected => Ok(()),
            other => Err(Error::RevisionGuard {
                op: "undo",
                path: path.to_path_buf(),
                expected: expected.to_string(),
                found: other.unwrap_or_else(|| "(absent)".into()),
            }),
        }
    }

    // ---------------------------------------------------------------------
    // State probes
    // ---------------------------------------------------------------------

    /// Metadata for `path`, or `None` if nothing exists there.
    fn metadata_opt(&self, path: &Path) -> Result<Option<ResourceMetadata>> {
        match self.store.metadata(path) {
            Ok(meta) => Ok(Some(meta)),
            Err(lattice_storage::Error::Io { ref source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Current content hash of `path`, or `None` if absent.
    fn current_hash(&self, path: &Path) -> Result<Option<String>> {
        Ok(self.metadata_opt(path)?.map(|m| m.revision.hash))
    }

    fn open_data_app(&self, path: &Path) -> Result<DataApp> {
        Ok(DataApp::open(&self.root.join(path))?)
    }

    fn package_revision(&self, path: &Path) -> Result<String> {
        Ok(self.open_data_app(path)?.package_revision()?)
    }

    fn ensure_package_revision(&self, path: &Path, expected: &str) -> Result<()> {
        let found = self.package_revision(path)?;
        if found == expected {
            Ok(())
        } else {
            Err(Error::StaleBaseRevision {
                path: path.to_path_buf(),
                expected: expected.to_string(),
                found,
            })
        }
    }

    fn ensure_canvas_revision(&self, path: &Path, expected: &str) -> Result<()> {
        canvas::validate_canvas_path(path)?;
        let canonical_root = self
            .root
            .canonicalize()
            .map_err(|source| Error::io(&self.root, source))?;
        let canonical_canvas = self
            .root
            .join(path)
            .canonicalize()
            .map_err(|source| Error::io(path, source))?;
        if !canonical_canvas.starts_with(&canonical_root) {
            return Err(Error::InvalidCanvas {
                path: path.to_path_buf(),
                reason: "canvas path escapes the workspace".into(),
            });
        }
        let meta = self.metadata_opt(path)?.ok_or_else(|| Error::NotFound {
            path: path.to_path_buf(),
        })?;
        if meta.is_dir {
            return Err(Error::NotFound {
                path: path.to_path_buf(),
            });
        }
        if meta.revision.hash == expected {
            Ok(())
        } else {
            Err(Error::StaleBaseRevision {
                path: path.to_path_buf(),
                expected: expected.to_string(),
                found: meta.revision.hash,
            })
        }
    }

    fn ensure_canvas_resource(&self, path: &Path) -> Result<()> {
        canvas::validate_path(path, "resource path")?;
        let canonical_root = self
            .root
            .canonicalize()
            .map_err(|source| Error::io(&self.root, source))?;
        let canonical_resource = self
            .root
            .join(path)
            .canonicalize()
            .map_err(|source| Error::io(path, source))?;
        if !canonical_resource.starts_with(&canonical_root) {
            return Err(Error::InvalidCanvas {
                path: path.to_path_buf(),
                reason: "resource path escapes the workspace".into(),
            });
        }
        let metadata = self.metadata_opt(path)?.ok_or_else(|| Error::NotFound {
            path: path.to_path_buf(),
        })?;
        if metadata.is_dir {
            return Err(Error::InvalidCanvas {
                path: path.to_path_buf(),
                reason: "resource path must identify a file".into(),
            });
        }
        Ok(())
    }

    fn guard_package_revision(&self, path: &Path, expected: &str) -> Result<()> {
        let found = self.package_revision(path)?;
        if found == expected {
            Ok(())
        } else {
            Err(Error::RevisionGuard {
                op: "undo",
                path: path.to_path_buf(),
                expected: expected.to_string(),
                found,
            })
        }
    }

    fn guard_row_present(&self, path: &Path, table: &str, row_id: &str) -> Result<()> {
        let app = self.open_data_app(path)?;
        if app.get_row(table, row_id)?.is_some() {
            Ok(())
        } else {
            Err(Error::RevisionGuard {
                op: "undo",
                path: path.to_path_buf(),
                expected: format!("row {row_id:?} present"),
                found: "(absent)".into(),
            })
        }
    }

    fn guard_row_absent(&self, path: &Path, table: &str, row_id: &str) -> Result<()> {
        let app = self.open_data_app(path)?;
        if app.get_row(table, row_id)?.is_none() {
            Ok(())
        } else {
            Err(Error::RevisionGuard {
                op: "undo",
                path: path.to_path_buf(),
                expected: format!("row {row_id:?} absent"),
                found: "(present)".into(),
            })
        }
    }

    fn guard_row_values(
        &self,
        path: &Path,
        table: &str,
        row_id: &str,
        expected_values: &std::collections::BTreeMap<String, lattice_data::CellValue>,
    ) -> Result<()> {
        let app = self.open_data_app(path)?;
        let row = app
            .get_row(table, row_id)?
            .ok_or_else(|| Error::RevisionGuard {
                op: "undo",
                path: path.to_path_buf(),
                expected: format!("row {row_id:?} present"),
                found: "(absent)".into(),
            })?;
        for (column, expected) in expected_values {
            match row.values.get(column) {
                Some(found) if found == expected => {}
                other => {
                    return Err(Error::RevisionGuard {
                        op: "undo",
                        path: path.to_path_buf(),
                        expected: format!("{column}={expected:?}"),
                        found: format!("{other:?}"),
                    });
                }
            }
        }
        Ok(())
    }

    fn read_view_opt(&self, view_path: &Path) -> Result<Option<String>> {
        match self.store.read(view_path) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(lattice_storage::Error::Io { ref source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(other) => Err(other.into()),
        }
    }
}

fn row_values_without_id(row: &Row) -> std::collections::BTreeMap<String, lattice_data::CellValue> {
    row.values
        .iter()
        .filter(|(key, _)| key.as_str() != "id")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
