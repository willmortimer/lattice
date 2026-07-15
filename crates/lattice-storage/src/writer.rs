use std::path::Path;

use crate::journal::RecoveryJournal;
use crate::revision::ResourceRevision;
use crate::store::{NativeWorkspaceStore, WorkspaceStore};
use crate::{Error, Result};

/// The one true write path: ties a [`NativeWorkspaceStore`] and a
/// [`RecoveryJournal`] together so every mutation is journaled before it
/// touches disk.
///
/// The order is exactly: compare the current on-disk revision against the
/// caller's `expected_base`, then journal the intent, then materialize
/// atomically, then mark the journal entry complete. A crash between the
/// journal append and the completion leaves a recoverable pending row.
pub struct BufferedWriter<'a> {
    store: &'a NativeWorkspaceStore,
    journal: &'a RecoveryJournal,
    session_id: String,
}

impl<'a> BufferedWriter<'a> {
    pub fn new(
        store: &'a NativeWorkspaceStore,
        journal: &'a RecoveryJournal,
        session_id: String,
    ) -> Self {
        BufferedWriter {
            store,
            journal,
            session_id,
        }
    }

    /// Write `data` to `path`. `expected_base` is the on-disk revision the
    /// caller believes is present (`None` when creating). If the disk
    /// disagrees, returns [`Error::RevisionMismatch`] and journals nothing.
    pub fn write(
        &self,
        path: &Path,
        data: &[u8],
        expected_base: Option<&ResourceRevision>,
    ) -> Result<ResourceRevision> {
        let current = self.store.current_revision(path)?;

        // Optimistic concurrency: the disk must be exactly what the caller
        // expected. Compare by content hash (absent == None).
        let expected_hash = expected_base.map(|r| r.hash.clone());
        let found_hash = current.as_ref().map(|r| r.hash.clone());
        if expected_hash != found_hash {
            return Err(Error::RevisionMismatch {
                path: path.to_path_buf(),
                expected: expected_hash,
                found: found_hash,
            });
        }

        let entry_id = self
            .journal
            .begin_write(path, expected_base, data, &self.session_id)?;
        let revision = self.store.write_atomic(path, data)?;
        self.journal.complete_write(entry_id, &revision)?;
        Ok(revision)
    }
}
