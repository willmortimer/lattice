//! In-memory registry of open collab sessions.

use std::collections::HashMap;
use std::path::Path;

use latticefs_core::{resource_stat_or_register, ResourceId};

use crate::error::{Error, Result};
use crate::id::parse_doc_resource_id;
use crate::session::CollabSession;

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
    /// `true` when this call created a new empty Yrs doc (vs re-open).
    pub created: bool,
}

/// Process-local map of open Yrs docs keyed by [`ResourceId`].
///
/// Not persisted; Y2 owns journal under `.lattice/collab/`.
#[derive(Default)]
pub struct CollabRegistry {
    sessions: HashMap<ResourceId, CollabSession>,
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
    /// When `workspace_root` and `path` are both set, registers/stats the path
    /// and requires the resolved ResourceId to match `doc_id`.
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
                let session = CollabSession::empty(doc_id);
                let snapshot = snapshot_of(&session);
                self.sessions.insert(doc_id, session);
                Ok(OpenedDoc {
                    snapshot,
                    created: true,
                })
            }
        }
    }

    /// Apply a binary Yrs update to an open session.
    pub fn apply_update(&mut self, doc_id_raw: &str, update: &[u8]) -> Result<CollabSnapshot> {
        let doc_id = parse_doc_resource_id(doc_id_raw)?;
        let session = self.sessions.get_mut(&doc_id).ok_or_else(|| {
            Error::SessionNotOpen {
                doc_id: doc_id.to_string(),
            }
        })?;
        session.apply_update_v1(update)?;
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
    pub fn close(&mut self, doc_id_raw: &str) -> Result<bool> {
        let doc_id = parse_doc_resource_id(doc_id_raw)?;
        let Some(session) = self.sessions.get_mut(&doc_id) else {
            return Err(Error::SessionNotOpen {
                doc_id: doc_id.to_string(),
            });
        };
        if session.release() {
            self.sessions.remove(&doc_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
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
    use std::fs;
    use yrs::updates::decoder::Decode;
    use yrs::updates::encoder::Encode;
    use yrs::{GetString, ReadTxn, Text, Transact, Update};


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

    #[test]
    fn open_apply_get_close_roundtrip() {
        let mut registry = CollabRegistry::new();
        let id = ResourceId::new();
        let opened = registry.open(&id.to_string(), None, None).unwrap();
        assert!(opened.created);
        assert_eq!(opened.snapshot.doc_id, id);

        let update = make_text_update("hello collab");
        let after = registry.apply_update(&id.to_string(), &update).unwrap();
        assert!(!after.state_vector.is_empty() || after.update.len() >= 2);

        let empty_sv = yrs::StateVector::default().encode_v1();
        let state = registry.get_state(&id.to_string(), &empty_sv).unwrap();
        assert!(!state.update.is_empty());

        // Apply recovered update into a fresh peer doc and read text.
        let peer = yrs::Doc::new();
        {
            let decoded = Update::decode_v1(&state.update).unwrap();
            let mut txn = peer.transact_mut();
            txn.apply_update(decoded).unwrap();
        }
        let text = peer.get_or_insert_text("content");
        let txn = peer.transact();
        assert_eq!(text.get_string(&txn), "hello collab");

        assert!(registry.close(&id.to_string()).unwrap());
        assert!(registry.is_empty());
        assert!(matches!(
            registry.apply_update(&id.to_string(), &update).unwrap_err(),
            Error::SessionNotOpen { .. }
        ));
    }

    #[test]
    fn reopen_increments_refcount() {
        let mut registry = CollabRegistry::new();
        let id = ResourceId::new();
        assert!(registry.open(&id.to_string(), None, None).unwrap().created);
        assert!(!registry.open(&id.to_string(), None, None).unwrap().created);
        assert!(!registry.close(&id.to_string()).unwrap());
        assert!(registry.contains(id));
        assert!(registry.close(&id.to_string()).unwrap());
        assert!(!registry.contains(id));
    }

    #[test]
    fn open_with_path_registers_and_matches() {
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("Notes.md");
        fs::write(&notes, "# hi\n").unwrap();

        // First call registers and yields a ResourceId we must use as doc_id.
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
}
