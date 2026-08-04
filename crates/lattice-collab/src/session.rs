//! Single in-memory Yrs document session.

use latticefs_core::ResourceId;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

use crate::error::{Error, Result};

/// Live Yrs document for one [`ResourceId`].
pub struct CollabSession {
    resource_id: ResourceId,
    doc: Doc,
    /// Number of outstanding opens; close decrements and drops at zero.
    open_count: usize,
}

impl CollabSession {
    /// Create an empty Yrs document for `resource_id`.
    pub fn empty(resource_id: ResourceId) -> Self {
        Self {
            resource_id,
            doc: Doc::new(),
            open_count: 1,
        }
    }

    /// Restore a session from an already-built [`Doc`] (snapshot ⊕ updates).
    pub fn restored(resource_id: ResourceId, doc: Doc) -> Self {
        Self {
            resource_id,
            doc,
            open_count: 1,
        }
    }

    pub fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    pub fn open_count(&self) -> usize {
        self.open_count
    }

    pub(crate) fn retain(&mut self) {
        self.open_count = self.open_count.saturating_add(1);
    }

    /// Decrement open count; returns `true` when the session should be removed.
    pub(crate) fn release(&mut self) -> bool {
        self.open_count = self.open_count.saturating_sub(1);
        self.open_count == 0
    }

    /// Encode the current state vector (lib0 v1).
    pub fn state_vector_v1(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.state_vector().encode_v1()
    }

    /// Encode updates the peer is missing given their state vector (lib0 v1).
    pub fn encode_missing_update_v1(&self, peer_state_vector: &[u8]) -> Result<Vec<u8>> {
        let sv = decode_state_vector(peer_state_vector)?;
        let txn = self.doc.transact();
        Ok(txn.encode_state_as_update_v1(&sv))
    }

    /// Full document state as an update from the empty state vector.
    pub fn encode_full_update_v1(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default())
    }

    /// Apply a lib0 v1 Yrs update.
    pub fn apply_update_v1(&mut self, update: &[u8]) -> Result<()> {
        let decoded = Update::decode_v1(update).map_err(|err| Error::Yrs {
            message: err.to_string(),
        })?;
        let mut txn = self.doc.transact_mut();
        txn.apply_update(decoded).map_err(|err| Error::Yrs {
            message: err.to_string(),
        })?;
        Ok(())
    }
}

fn decode_state_vector(bytes: &[u8]) -> Result<StateVector> {
    StateVector::decode_v1(bytes).map_err(|err| Error::Yrs {
        message: format!("invalid state vector: {err}"),
    })
}
