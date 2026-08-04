//! In-memory registry of open collab sessions with optional disk journal.

use std::collections::HashMap;
use std::path::Path;

use latticefs_core::{resource_stat_or_register, ResourceId};
use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use crate::error::{Error, Result};
use crate::id::parse_doc_resource_id;
use crate::journal::{
    append_update, compact_to_snapshot, journal_dir, journal_exists, read_snapshot, read_updates,
};
use crate::session::CollabSession;

/// Auto-compact the append log after this many persisted updates since open
/// (or since the last compaction).
const COMPACT_AFTER_UPDATES: usize = 32;

/// Snapshot returned after open / get-state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollabSnapshot {
    pub doc_id: ResourceId,
    pub state_vector: Vec<u8>,
    /// Yrs update bytes the caller should apply (full state on open; diff on get).
    pub update: Vec<u8>,
}

/// Result of opening a document session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedDoc {
    pub snapshot: CollabSnapshot,
    /// `true` when this call created a new empty Yrs doc (no prior memory/journal).
    pub created: bool,
}

/// Process-local map of open Yrs docs keyed by [`ResourceId`].
///
/// When `workspace_root` is supplied, sessions reopen from
/// `.lattice/collab/<uuid>/` and updates are appended before in-memory apply.
#[derive(Default)]
pub struct CollabRegistry {
    sessions: HashMap<ResourceId, CollabSession>,
    /// Updates appended since open / last compaction (per doc), for periodic fold.
    pending_since_compact: HashMap<ResourceId, usize>,
}

impl CollabRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn contains(&self, doc_id: ResourceId) -> bool {
        self.sessions.contains_key(&doc_id)
    }

    /// Open (or re-open) a session for `doc_id`.
    ///
    /// When `workspace_root` is set and a journal exists, loads snapshot ⊕ updates
    /// before serving. When `workspace_root` and `path` are both set, registers/stats
    /// the path and requires the resolved ResourceId to match `doc_id`.
    pub fn open(
        &mut self,
        doc_id_raw: &str,
        workspace_root: Option<&Path>,
        path: Option<&str>,
    ) -> Result<OpenedDoc> {
        let doc_id = resolve_open_doc_id(doc_id_raw, workspace_root, path)?;
        match self.sessions.get_mut(&doc_id) {
            Some(session) => {
                session.retain();
                Ok(OpenedDoc {
                    snapshot: snapshot_of(session),
                    created: false,
                })
            }
            None => {
                let (session, created) = match workspace_root {
                    Some(root) => load_or_create(root, doc_id)?,
                    None => (CollabSession::empty(doc_id), true),
                };
                let snapshot = snapshot_of(&session);
                self.sessions.insert(doc_id, session);
                self.pending_since_compact.insert(doc_id, 0);
                Ok(OpenedDoc { snapshot, created })
            }
        }
    }

    /// Apply a binary Yrs update to an open session.
    ///
    /// When `workspace_root` is set, the update is appended to the journal before
    /// the in-memory apply. Periodic compaction may fold the log into a snapshot.
    pub fn apply_update(
        &mut self,
        doc_id_raw: &str,
        update: &[u8],
        workspace_root: Option<&Path>,
    ) -> Result<CollabSnapshot> {
        let doc_id = parse_doc_resource_id(doc_id_raw)?;
        // Confirm the session exists before durable append so we never journal
        // updates for a closed doc_id.
        if !self.sessions.contains_key(&doc_id) {
            return Err(Error::SessionNotOpen {
                doc_id: doc_id.to_string(),
            });
        }
        if let Some(root) = workspace_root {
            append_update(&journal_dir(root, doc_id), update)?;
        }
        let session = self
            .sessions
            .get_mut(&doc_id)
            .expect("session checked above");
        session.apply_update_v1(update)?;
        if let Some(root) = workspace_root {
            let pending = self.pending_since_compact.entry(doc_id).or_insert(0);
            *pending = pending.saturating_add(1);
            if *pending >= COMPACT_AFTER_UPDATES {
                let full = session.encode_full_update_v1();
                compact_to_snapshot(&journal_dir(root, doc_id), &full)?;
                *pending = 0;
            }
        }
        Ok(snapshot_of(session))
    }

    /// Fold the live document into `snapshot.bin` and truncate `updates.bin`.
    ///
    /// Requires an open session and a workspace root.
    pub fn compact(&mut self, doc_id_raw: &str, workspace_root: &Path) -> Result<CollabSnapshot> {
        let doc_id = parse_doc_resource_id(doc_id_raw)?;
        let session = self.sessions.get(&doc_id).ok_or_else(|| Error::SessionNotOpen {
            doc_id: doc_id.to_string(),
        })?;
        let full = session.encode_full_update_v1();
        compact_to_snapshot(&journal_dir(workspace_root, doc_id), &full)?;
        self.pending_since_compact.insert(doc_id, 0);
        Ok(snapshot_of(session))
    }

    /// Return server state vector plus updates missing from the client SV.
    pub fn get_state(
        &self,
        doc_id_raw: &str,
        client_state_vector: &[u8],
    ) -> Result<CollabSnapshot> {
        let doc_id = parse_doc_resource_id(doc_id_raw)?;
        let session = self.sessions.get(&doc_id).ok_or_else(|| Error::SessionNotOpen {
            doc_id: doc_id.to_string(),
        })?;
        let update = session.encode_missing_update_v1(client_state_vector)?;
        Ok(CollabSnapshot {
            doc_id,
            state_vector: session.state_vector_v1(),
            update,
        })
    }

    /// Close one open reference; removes the session when the last open ends.
    ///
    /// When the last reference drops and `workspace_root` is set, writes a snapshot
    /// and truncates the update log.
    pub fn close(
        &mut self,
        doc_id_raw: &str,
        workspace_root: Option<&Path>,
    ) -> Result<bool> {
        let doc_id = parse_doc_resource_id(doc_id_raw)?;
        let Some(session) = self.sessions.get_mut(&doc_id) else {
            return Err(Error::SessionNotOpen {
                doc_id: doc_id.to_string(),
            });
        };
        if session.release() {
            if let Some(root) = workspace_root {
                let full = session.encode_full_update_v1();
                compact_to_snapshot(&journal_dir(root, doc_id), &full)?;
            }
            self.sessions.remove(&doc_id);
            self.pending_since_compact.remove(&doc_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn load_or_create(workspace_root: &Path, doc_id: ResourceId) -> Result<(CollabSession, bool)> {
    let dir = journal_dir(workspace_root, doc_id);
    if !journal_exists(&dir) {
        return Ok((CollabSession::empty(doc_id), true));
    }
    let doc = Doc::new();
    if let Some(snapshot) = read_snapshot(&dir)? {
        if !snapshot.is_empty() {
            apply_raw(&doc, &snapshot)?;
        }
    }
    for update in read_updates(&dir)? {
        apply_raw(&doc, &update)?;
    }
    Ok((CollabSession::restored(doc_id, doc), false))
}

fn apply_raw(doc: &Doc, update: &[u8]) -> Result<()> {
    let decoded = Update::decode_v1(update).map_err(|err| Error::Yrs {
        message: err.to_string(),
    })?;
    let mut txn = doc.transact_mut();
    txn.apply_update(decoded).map_err(|err| Error::Yrs {
        message: err.to_string(),
    })?;
    Ok(())
}

fn snapshot_of(session: &CollabSession) -> CollabSnapshot {
    CollabSnapshot {
        doc_id: session.resource_id(),
        state_vector: session.state_vector_v1(),
        update: session.encode_full_update_v1(),
    }
}

fn resolve_open_doc_id(
    doc_id_raw: &str,
    workspace_root: Option<&Path>,
    path: Option<&str>,
) -> Result<ResourceId> {
    let doc_id = parse_doc_resource_id(doc_id_raw)?;
    let (Some(root), Some(rel)) = (workspace_root, path.filter(|p| !p.is_empty())) else {
        return Ok(doc_id);
    };
    let stat = resource_stat_or_register(root, rel).map_err(|err| Error::ResourceResolve {
        message: err.to_string(),
    })?;
    if stat.resource_id != doc_id {
        return Err(Error::ResourceIdMismatch {
            requested: doc_id.to_string(),
            resolved: stat.resource_id.to_string(),
        });
    }
    Ok(doc_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{read_updates, SNAPSHOT_FILENAME, UPDATES_FILENAME};
    use std::fs;
    use yrs::updates::encoder::Encode;
    use yrs::{GetString, ReadTxn, Text, Transact};

    fn make_text_update(text: &str) -> Vec<u8> {
        let doc = yrs::Doc::new();
        let shared = doc.get_or_insert_text("content");
        {
            let mut txn = doc.transact_mut();
            shared.push(&mut txn, text);
        }
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&yrs::StateVector::default())
    }

    /// Incremental updates from one client doc so replay concatenates in order.
    fn make_incremental_updates(fragments: &[&str]) -> Vec<Vec<u8>> {
        let doc = yrs::Doc::new();
        let shared = doc.get_or_insert_text("content");
        let mut updates = Vec::new();
        let mut prev_sv = yrs::StateVector::default();
        for fragment in fragments {
            {
                let mut txn = doc.transact_mut();
                shared.push(&mut txn, *fragment);
            }
            let txn = doc.transact();
            updates.push(txn.encode_state_as_update_v1(&prev_sv));
            prev_sv = txn.state_vector();
        }
        updates
    }

    fn text_of_snapshot(update: &[u8]) -> String {
        let peer = yrs::Doc::new();
        {
            let decoded = Update::decode_v1(update).unwrap();
            let mut txn = peer.transact_mut();
            txn.apply_update(decoded).unwrap();
        }
        let text = peer.get_or_insert_text("content");
        let txn = peer.transact();
        text.get_string(&txn)
    }

    #[test]
    fn open_apply_get_close_roundtrip() {
        let mut registry = CollabRegistry::new();
        let id = ResourceId::new();
        let opened = registry.open(&id.to_string(), None, None).unwrap();
        assert!(opened.created);
        assert_eq!(opened.snapshot.doc_id, id);

        let update = make_text_update("hello collab");
        let after = registry
            .apply_update(&id.to_string(), &update, None)
            .unwrap();
        assert!(!after.state_vector.is_empty() || after.update.len() >= 2);

        let empty_sv = yrs::StateVector::default().encode_v1();
        let state = registry.get_state(&id.to_string(), &empty_sv).unwrap();
        assert!(!state.update.is_empty());

        assert_eq!(text_of_snapshot(&state.update), "hello collab");

        assert!(registry.close(&id.to_string(), None).unwrap());
        assert!(registry.is_empty());
        assert!(matches!(
            registry
                .apply_update(&id.to_string(), &update, None)
                .unwrap_err(),
            Error::SessionNotOpen { .. }
        ));
    }

    #[test]
    fn reopen_increments_refcount() {
        let mut registry = CollabRegistry::new();
        let id = ResourceId::new();
        assert!(registry.open(&id.to_string(), None, None).unwrap().created);
        assert!(!registry.open(&id.to_string(), None, None).unwrap().created);
        assert!(!registry.close(&id.to_string(), None).unwrap());
        assert!(registry.contains(id));
        assert!(registry.close(&id.to_string(), None).unwrap());
        assert!(!registry.contains(id));
    }

    #[test]
    fn open_with_path_registers_and_matches() {
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("Notes.md");
        fs::write(&notes, "# hi\n").unwrap();

        let stat = resource_stat_or_register(dir.path(), "Notes.md").unwrap();
        let mut registry = CollabRegistry::new();
        let opened = registry
            .open(
                &stat.resource_id.to_string(),
                Some(dir.path()),
                Some("Notes.md"),
            )
            .unwrap();
        assert!(opened.created);
        assert_eq!(opened.snapshot.doc_id, stat.resource_id);
    }

    #[test]
    fn open_rejects_path_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Notes.md"), "# hi\n").unwrap();
        let stat = resource_stat_or_register(dir.path(), "Notes.md").unwrap();
        let other = ResourceId::new();
        assert_ne!(stat.resource_id, other);

        let mut registry = CollabRegistry::new();
        let err = registry
            .open(&other.to_string(), Some(dir.path()), Some("Notes.md"))
            .unwrap_err();
        assert!(matches!(err, Error::ResourceIdMismatch { .. }));
    }

    #[test]
    fn rejects_path_scheme_doc_id() {
        let mut registry = CollabRegistry::new();
        let err = registry
            .open("path:Notes.md", None, None)
            .unwrap_err();
        assert!(matches!(err, Error::InvalidDocId { .. }));
    }

    #[test]
    fn crash_reopen_restores_doc_from_journal() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let id = ResourceId::new();
        let id_str = id.to_string();

        let mut registry = CollabRegistry::new();
        assert!(registry.open(&id_str, Some(root), None).unwrap().created);

        let updates = make_incremental_updates(&["alpha", " beta", " gamma"]);
        for update in &updates {
            registry
                .apply_update(&id_str, update, Some(root))
                .unwrap();
        }
        let before = registry
            .get_state(&id_str, &yrs::StateVector::default().encode_v1())
            .unwrap();
        let before_text = text_of_snapshot(&before.update);
        let before_sv = before.state_vector.clone();

        // Simulate process death: drop registry without close compaction.
        drop(registry);

        let jdir = journal_dir(root, id);
        assert!(jdir.join(UPDATES_FILENAME).is_file());
        assert!(!read_updates(&jdir).unwrap().is_empty());

        let mut registry = CollabRegistry::new();
        let opened = registry.open(&id_str, Some(root), None).unwrap();
        assert!(!opened.created);
        assert_eq!(opened.snapshot.state_vector, before_sv);
        assert_eq!(text_of_snapshot(&opened.snapshot.update), before_text);
        assert_eq!(before_text, "alpha beta gamma");
    }

    #[test]
    fn close_compacts_snapshot_and_truncates_log() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let id = ResourceId::new();
        let id_str = id.to_string();

        let mut registry = CollabRegistry::new();
        registry.open(&id_str, Some(root), None).unwrap();
        registry
            .apply_update(&id_str, &make_text_update("persist"), Some(root))
            .unwrap();
        assert!(registry.close(&id_str, Some(root)).unwrap());

        let jdir = journal_dir(root, id);
        assert!(jdir.join(SNAPSHOT_FILENAME).is_file());
        assert!(read_updates(&jdir).unwrap().is_empty());

        let mut registry = CollabRegistry::new();
        let opened = registry.open(&id_str, Some(root), None).unwrap();
        assert!(!opened.created);
        assert_eq!(text_of_snapshot(&opened.snapshot.update), "persist");
    }

    #[test]
    fn compact_api_folds_updates_into_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let id = ResourceId::new();
        let id_str = id.to_string();

        let mut registry = CollabRegistry::new();
        registry.open(&id_str, Some(root), None).unwrap();
        for update in make_incremental_updates(&["one", " two", " three"]) {
            registry
                .apply_update(&id_str, &update, Some(root))
                .unwrap();
        }
        let jdir = journal_dir(root, id);
        assert!(!read_updates(&jdir).unwrap().is_empty());

        let snap = registry.compact(&id_str, root).unwrap();
        assert!(read_updates(&jdir).unwrap().is_empty());
        assert!(jdir.join(SNAPSHOT_FILENAME).is_file());
        assert_eq!(text_of_snapshot(&snap.update), "one two three");
    }

    #[test]
    fn append_before_apply_survives_failed_memory_drop() {
        // Durability contract: journal frames exist after apply returns, so a
        // subsequent drop of the in-memory registry still reopens equal state.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let id = ResourceId::new();
        let id_str = id.to_string();

        let mut registry = CollabRegistry::new();
        registry.open(&id_str, Some(root), None).unwrap();
        let fragments: Vec<String> = (0..5).map(|i| format!("u{i}")).collect();
        let refs: Vec<&str> = fragments.iter().map(String::as_str).collect();
        for update in make_incremental_updates(&refs) {
            registry
                .apply_update(&id_str, &update, Some(root))
                .unwrap();
        }
        let expected = registry
            .get_state(&id_str, &yrs::StateVector::default().encode_v1())
            .unwrap();

        drop(registry);

        let mut registry = CollabRegistry::new();
        let opened = registry.open(&id_str, Some(root), None).unwrap();
        assert_eq!(opened.snapshot.state_vector, expected.state_vector);
        assert_eq!(
            text_of_snapshot(&opened.snapshot.update),
            text_of_snapshot(&expected.update)
        );
    }
}
