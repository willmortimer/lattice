//! Thin remote store for opaque Yrs snapshot bytes keyed by page [`ResourceId`].
//!
//! Transport choice (S8): reuse cloud `PUT/GET /v1/blobs/{resource_id}` with a
//! deterministic **sidecar** ResourceId so Markdown open-format blobs (S5) are
//! never overwritten. Local journal remains source of truth; remote carries a
//! full lib0 v1 update from the empty state vector for peer catch-up.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use latticefs_core::{ContentHash, ResourceId};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Magic prefix for remote collab snapshot payloads (`LYRS`).
pub const REMOTE_SNAPSHOT_MAGIC: &[u8; 4] = b"LYRS";
/// Payload format version.
pub const REMOTE_SNAPSHOT_VERSION: u8 = 1;

/// Fixed UUID namespace for sidecar derivation (URL namespace + stable name).
const SIDECAR_NAME_PREFIX: &str = "lattice.collab.yrs-snapshot.v1:";

/// Result of a successful remote put.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePutResult {
    pub page_id: ResourceId,
    pub sidecar_id: ResourceId,
    pub content_hash: String,
}

/// Pulled remote snapshot (Yrs update bytes + content hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePullResult {
    pub page_id: ResourceId,
    pub sidecar_id: ResourceId,
    pub update: Vec<u8>,
    pub content_hash: String,
}

/// Opaque store for Yrs snapshot blobs keyed by page ResourceId.
pub trait YrsRemoteStore {
    /// Persist a full Yrs update (lib0 v1 from empty SV) for `page_id`.
    ///
    /// `if_match` is the prior content-hash hex (optional optimistic concurrency).
    fn put_snapshot(
        &self,
        page_id: ResourceId,
        yrs_update: &[u8],
        if_match: Option<&str>,
    ) -> Result<RemotePutResult>;

    /// Fetch the latest snapshot for `page_id`, if any.
    fn get_snapshot(&self, page_id: ResourceId) -> Result<Option<RemotePullResult>>;
}

/// Deterministic sidecar ResourceId for the collab snapshot of a page.
///
/// Sidecar blobs live alongside Markdown sync heads without colliding on the
/// page's own ResourceId.
pub fn collab_snapshot_resource_id(page_id: ResourceId) -> ResourceId {
    let name = format!("{SIDECAR_NAME_PREFIX}{page_id}");
    ResourceId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, name.as_bytes()))
}

/// Encode a remote payload wrapping a Yrs full-state update.
pub fn encode_remote_snapshot(page_id: ResourceId, yrs_update: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 4 + 16 + yrs_update.len());
    out.extend_from_slice(REMOTE_SNAPSHOT_MAGIC);
    out.push(REMOTE_SNAPSHOT_VERSION);
    out.push(0); // flags
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(page_id.as_uuid().as_bytes());
    out.extend_from_slice(yrs_update);
    out
}

/// Decode a remote payload; verifies magic, version, and embedded page id.
pub fn decode_remote_snapshot(page_id: ResourceId, bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < 4 + 4 + 16 {
        return Err(Error::RemotePayload {
            message: "remote snapshot too short".into(),
        });
    }
    if &bytes[0..4] != REMOTE_SNAPSHOT_MAGIC.as_slice() {
        return Err(Error::RemotePayload {
            message: "remote snapshot magic mismatch".into(),
        });
    }
    if bytes[4] != REMOTE_SNAPSHOT_VERSION {
        return Err(Error::RemotePayload {
            message: format!("unsupported remote snapshot version {}", bytes[4]),
        });
    }
    let embedded = Uuid::from_bytes(
        bytes[8..24]
            .try_into()
            .expect("16-byte page id slice"),
    );
    let embedded_id = ResourceId::from_uuid(embedded);
    if embedded_id != page_id {
        return Err(Error::RemotePayload {
            message: format!(
                "remote snapshot page_id mismatch: embedded {embedded_id}, expected {page_id}"
            ),
        });
    }
    Ok(bytes[24..].to_vec())
}

fn content_hash_hex(data: &[u8]) -> Result<String> {
    let hash = ContentHash::from_bytes(data).map_err(|err| Error::RemotePayload {
        message: err.to_string(),
    })?;
    Ok(hash
        .as_str()
        .strip_prefix("sha256:")
        .unwrap_or(hash.as_str())
        .to_string())
}

/// Process-local remote store for tests and offline stubs.
#[derive(Clone, Default)]
pub struct MemoryYrsRemoteStore {
    inner: Arc<Mutex<HashMap<ResourceId, (Vec<u8>, String)>>>,
}

impl MemoryYrsRemoteStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl YrsRemoteStore for MemoryYrsRemoteStore {
    fn put_snapshot(
        &self,
        page_id: ResourceId,
        yrs_update: &[u8],
        if_match: Option<&str>,
    ) -> Result<RemotePutResult> {
        let sidecar_id = collab_snapshot_resource_id(page_id);
        let payload = encode_remote_snapshot(page_id, yrs_update);
        let hash = content_hash_hex(&payload)?;
        let mut guard = self.inner.lock().map_err(|_| Error::RemotePayload {
            message: "memory remote store poisoned".into(),
        })?;
        if let Some(expected) = if_match.map(str::trim).filter(|s| !s.is_empty()) {
            match guard.get(&sidecar_id) {
                Some((_, current)) if current == expected => {}
                Some((_, current)) => {
                    return Err(Error::RemoteConflict {
                        expected: expected.to_string(),
                        actual: current.clone(),
                    });
                }
                None => {
                    return Err(Error::RemoteConflict {
                        expected: expected.to_string(),
                        actual: "missing".into(),
                    });
                }
            }
        }
        guard.insert(sidecar_id, (payload, hash.clone()));
        Ok(RemotePutResult {
            page_id,
            sidecar_id,
            content_hash: hash,
        })
    }

    fn get_snapshot(&self, page_id: ResourceId) -> Result<Option<RemotePullResult>> {
        let sidecar_id = collab_snapshot_resource_id(page_id);
        let guard = self.inner.lock().map_err(|_| Error::RemotePayload {
            message: "memory remote store poisoned".into(),
        })?;
        let Some((payload, hash)) = guard.get(&sidecar_id) else {
            return Ok(None);
        };
        let update = decode_remote_snapshot(page_id, payload)?;
        Ok(Some(RemotePullResult {
            page_id,
            sidecar_id,
            update,
            content_hash: hash.clone(),
        }))
    }
}

/// Encode → put → get → decode round-trip helper used by tests and stubs.
pub fn exchange_snapshot_roundtrip<S: YrsRemoteStore>(
    store: &S,
    page_id: ResourceId,
    yrs_update: &[u8],
) -> Result<Vec<u8>> {
    store.put_snapshot(page_id, yrs_update, None)?;
    let pulled = store.get_snapshot(page_id)?.ok_or_else(|| Error::RemotePayload {
        message: "snapshot missing after put".into(),
    })?;
    Ok(pulled.update)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CollabRegistry;
    use yrs::updates::decoder::Decode;
    use yrs::updates::encoder::Encode;
    use yrs::{Doc, GetString, ReadTxn, Text, Transact, Update};

    fn make_text_update(text: &str) -> Vec<u8> {
        let doc = Doc::new();
        let shared = doc.get_or_insert_text("content");
        {
            let mut txn = doc.transact_mut();
            shared.push(&mut txn, text);
        }
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&yrs::StateVector::default())
    }

    fn text_of(update: &[u8]) -> String {
        let peer = Doc::new();
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
    fn sidecar_id_is_stable_and_distinct() {
        let page = ResourceId::new();
        let a = collab_snapshot_resource_id(page);
        let b = collab_snapshot_resource_id(page);
        assert_eq!(a, b);
        assert_ne!(a, page);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let page = ResourceId::new();
        let update = make_text_update("hello remote");
        let payload = encode_remote_snapshot(page, &update);
        let decoded = decode_remote_snapshot(page, &payload).unwrap();
        assert_eq!(decoded, update);
        assert_eq!(text_of(&decoded), "hello remote");
    }

    #[test]
    fn memory_store_rejects_stale_if_match() {
        let store = MemoryYrsRemoteStore::new();
        let page = ResourceId::new();
        let first = store
            .put_snapshot(page, &make_text_update("a"), None)
            .unwrap();
        let err = store
            .put_snapshot(page, &make_text_update("b"), Some("deadbeef"))
            .unwrap_err();
        assert!(matches!(err, Error::RemoteConflict { .. }));
        let ok = store
            .put_snapshot(page, &make_text_update("b"), Some(&first.content_hash))
            .unwrap();
        assert_ne!(ok.content_hash, first.content_hash);
    }

    #[test]
    fn two_sessions_exchange_via_remote_store() {
        let store = MemoryYrsRemoteStore::new();
        let page = ResourceId::new();
        let page_str = page.to_string();

        let mut sender = CollabRegistry::new();
        sender.open(&page_str, None, None).unwrap();
        let update = make_text_update("peer-a typed");
        sender.apply_update(&page_str, &update, None).unwrap();
        let full = sender
            .get_state(&page_str, &yrs::StateVector::default().encode_v1())
            .unwrap()
            .update;

        let exchanged = exchange_snapshot_roundtrip(&store, page, &full).unwrap();
        assert_eq!(text_of(&exchanged), "peer-a typed");

        let mut receiver = CollabRegistry::new();
        receiver.open(&page_str, None, None).unwrap();
        receiver
            .apply_update(&page_str, &exchanged, None)
            .unwrap();
        let after = receiver
            .get_state(&page_str, &yrs::StateVector::default().encode_v1())
            .unwrap();
        assert_eq!(text_of(&after.update), "peer-a typed");
    }
}
