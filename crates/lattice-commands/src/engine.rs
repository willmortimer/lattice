use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lattice_core::Workspace;
use lattice_storage::{
    BufferedWriter, NativeWorkspaceStore, RecoveryJournal, ResourceMetadata, WorkspaceStore,
};

use crate::command::{
    file_name, Command, CommandOutcome, HistoryEntry, Transaction, TransactionReceipt, UndoReport,
};
use crate::history::{unix_now, unix_to_system, HistoryStore};
use crate::trash::{dispose, TrashPolicy};
use crate::{Error, Result};

/// One applied command, carrying everything history needs to reverse it.
struct AppliedOp {
    forward: Command,
    inverse: Command,
    /// Bytes displaced by the command (previous page content, deleted file
    /// content). These, not the JSON, are authoritative for restoration.
    prior_content: Option<Vec<u8>>,
    /// Content hash of the command's target after it applied; `None` for
    /// deletes. Checked by the undo guard.
    resulting_revision: Option<String>,
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
        let rowid = self.history.insert_transaction(
            &tx.id,
            &tx.summary,
            unix_now(),
            tx.idempotency_key.as_deref(),
        )?;
        for (seq, op) in applied.iter().enumerate() {
            self.history.insert_operation(
                rowid,
                seq as i64,
                &serde_json::to_string(&op.forward)?,
                &serde_json::to_string(&op.inverse)?,
                op.prior_content.as_deref(),
                op.resulting_revision.as_deref(),
            )?;
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
            Command::PageCreate { path, .. } => match self.metadata_opt(path)? {
                None => Ok(()),
                Some(_) => Err(Error::AlreadyExists { path: path.clone() }),
            },
            Command::PageUpdate {
                path,
                base_revision,
                ..
            } => {
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
        }
    }

    // ---------------------------------------------------------------------
    // Forward application
    // ---------------------------------------------------------------------

    fn writer(&self) -> BufferedWriter<'_> {
        BufferedWriter::new(&self.store, &self.journal, self.session_id.clone())
    }

    fn apply_one(&self, command: &Command) -> Result<AppliedOp> {
        match command {
            Command::PageCreate { path, content } => {
                let revision = self.writer().write(path, content.as_bytes(), None)?;
                Ok(AppliedOp {
                    forward: command.clone(),
                    inverse: Command::ResourceDelete { path: path.clone() },
                    prior_content: None,
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
                    resulting_revision: None,
                })
            }
        }
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
            // Inverse of ResourceDelete (file): re-materialize the bytes.
            Command::PageCreate { path, content } => {
                let bytes = prior_content
                    .map(<[u8]>::to_vec)
                    .unwrap_or_else(|| content.clone().into_bytes());
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
            Command::PageCreate { .. } | Command::PageUpdate { .. } => {
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
}
