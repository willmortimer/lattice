//! Thin remote store for opaque Yrs snapshot bytes keyed by page [`ResourceId`].
//!
//! Transport choice (S8): reuse cloud `PUT/GET /v1/blobs/{resource_id}` with a
//! deterministic **sidecar** ResourceId so Markdown open-format blobs (S5) are
//! never overwritten. Local journal remains source of truth; remote carries a
//! full lib0 v1 update from the empty state vector for peer catch-up.
//!
//! Append-only update logs (`LYRL`) let peers catch up without replacing the
//! whole snapshot on every poll. Compaction back to `LYRS` is caller-driven.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use latticefs_core::{ContentHash, ResourceId};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Magic prefix for remote collab snapshot payloads (`LYRS`).
pub const REMOTE_SNAPSHOT_MAGIC: &[u8; 4] = b"LYRS";
/// Payload format version.
pub const REMOTE_SNAPSHOT_VERSION: u8 = 1;

/// Magic prefix for remote collab append-only update logs (`LYRL`).
pub const REMOTE_LOG_MAGIC: &[u8; 4] = b"LYRL";
/// Append-log payload format version.
pub const REMOTE_LOG_VERSION: u8 = 1;

/// Maximum number of lib0 updates in one log blob before compaction is required.
pub const REMOTE_LOG_MAX_UPDATES: usize = 256;
/// Maximum total bytes of lib0 update payloads in one log blob.
pub const REMOTE_LOG_MAX_BYTES: usize = 1024 * 1024;

/// Fixed UUID namespace for snapshot sidecar derivation (URL namespace + stable name).
const SIDECAR_NAME_PREFIX: &str = "lattice.collab.yrs-snapshot.v1:";
/// Fixed UUID namespace for append-log sidecar derivation.
const LOG_SIDECAR_NAME_PREFIX: &str = "lattice.collab.yrs-log.v1:";

/// Size of the fixed LYRL header (magic through `base_hash`).
const REMOTE_LOG_HEADER_LEN: usize = 4 + 1 + 1 + 2 + 16 + 32;

/// Unknown snapshot base: 32 zero bytes.
pub const REMOTE_LOG_UNKNOWN_BASE_HASH: [u8; 32] = [0u8; 32];

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

/// Decoded append-only remote log payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLogDecoded {
    pub base_hash: [u8; 32],
    pub updates: Vec<Vec<u8>>,
}

/// Pulled remote append log (updates + content hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLogPullResult {
    pub page_id: ResourceId,
    pub sidecar_id: ResourceId,
    pub base_hash: [u8; 32],
    pub updates: Vec<Vec<u8>>,
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

/// Opaque store for Yrs append-only update logs keyed by page ResourceId.
pub trait YrsRemoteLogStore {
    /// Persist an append-only log for `page_id`.
    ///
    /// `if_match` is the prior content-hash hex (optional optimistic concurrency).
    fn put_log(
        &self,
        page_id: ResourceId,
        base_hash: [u8; 32],
        updates: &[&[u8]],
        if_match: Option<&str>,
    ) -> Result<RemotePutResult>;

    /// Fetch the latest log for `page_id`, if any.
    fn get_log(&self, page_id: ResourceId) -> Result<Option<RemoteLogPullResult>>;
}

/// Deterministic sidecar ResourceId for the collab snapshot of a page.
///
/// Sidecar blobs live alongside Markdown sync heads without colliding on the
/// page's own ResourceId.
pub fn collab_snapshot_resource_id(page_id: ResourceId) -> ResourceId {
    let name = format!("{SIDECAR_NAME_PREFIX}{page_id}");
    ResourceId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, name.as_bytes()))
}

/// Deterministic sidecar ResourceId for the collab append log of a page.
pub fn collab_log_resource_id(page_id: ResourceId) -> ResourceId {
    let name = format!("{LOG_SIDECAR_NAME_PREFIX}{page_id}");
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

/// Encode a remote append-only log wrapping length-prefixed lib0 v1 updates.
pub fn encode_remote_log(
    page_id: ResourceId,
    base_hash: [u8; 32],
    updates: &[&[u8]],
) -> Vec<u8> {
    let updates_len: usize = updates
        .iter()
        .map(|u| 4 + u.len())
        .sum();
    let mut out = Vec::with_capacity(REMOTE_LOG_HEADER_LEN + updates_len);
    out.extend_from_slice(REMOTE_LOG_MAGIC);
    out.push(REMOTE_LOG_VERSION);
    out.push(0); // flags
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(page_id.as_uuid().as_bytes());
    out.extend_from_slice(&base_hash);
    for update in updates {
        let len = u32::try_from(update.len()).expect("update length fits u32");
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(update);
    }
    out
}

/// Decode a remote append log; verifies magic, version, and embedded page id.
pub fn decode_remote_log(page_id: ResourceId, bytes: &[u8]) -> Result<RemoteLogDecoded> {
    if bytes.len() < REMOTE_LOG_HEADER_LEN {
        return Err(Error::RemotePayload {
            message: "remote log too short".into(),
        });
    }
    if &bytes[0..4] != REMOTE_LOG_MAGIC.as_slice() {
        return Err(Error::RemotePayload {
            message: "remote log magic mismatch".into(),
        });
    }
    if bytes[4] != REMOTE_LOG_VERSION {
        return Err(Error::RemotePayload {
            message: format!("unsupported remote log version {}", bytes[4]),
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
                "remote log page_id mismatch: embedded {embedded_id}, expected {page_id}"
            ),
        });
    }
    let base_hash: [u8; 32] = bytes[24..56]
        .try_into()
        .expect("32-byte base_hash slice");
    let mut updates = Vec::new();
    let mut offset = REMOTE_LOG_HEADER_LEN;
    while offset < bytes.len() {
        if offset + 4 > bytes.len() {
            return Err(Error::RemotePayload {
                message: "remote log truncated update length".into(),
            });
        }
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32")) as usize;
        offset += 4;
        if offset + len > bytes.len() {
            return Err(Error::RemotePayload {
                message: "remote log truncated update payload".into(),
            });
        }
        updates.push(bytes[offset..offset + len].to_vec());
        offset += len;
    }
    Ok(RemoteLogDecoded { base_hash, updates })
}

fn log_payload_byte_count(updates: &[Vec<u8>]) -> usize {
    updates.iter().map(|u| u.len()).sum()
}

fn check_log_limits(updates: &[Vec<u8>]) -> Result<()> {
    let update_count = updates.len();
    let byte_count = log_payload_byte_count(updates);
    if update_count > REMOTE_LOG_MAX_UPDATES || byte_count > REMOTE_LOG_MAX_BYTES {
        return Err(Error::LogNeedsCompact {
            update_count,
            byte_count,
        });
    }
    Ok(())
}

/// Append one lib0 v1 update to an existing log blob (or start a new log when empty).
///
/// Returns [`Error::LogNeedsCompact`] when limits are exceeded; callers should
/// compact to an `LYRS` snapshot and start a fresh log.
pub fn append_update(page_id: ResourceId, existing: &[u8], new_update: &[u8]) -> Result<Vec<u8>> {
    let mut decoded = if existing.is_empty() {
        RemoteLogDecoded {
            base_hash: REMOTE_LOG_UNKNOWN_BASE_HASH,
            updates: Vec::new(),
        }
    } else {
        decode_remote_log(page_id, existing)?
    };
    decoded.updates.push(new_update.to_vec());
    check_log_limits(&decoded.updates)?;
    Ok(encode_remote_log(
        page_id,
        decoded.base_hash,
        &decoded
            .updates
            .iter()
            .map(|u| u.as_slice())
            .collect::<Vec<_>>(),
    ))
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

/// Process-local append-log store for tests and offline stubs.
#[derive(Clone, Default)]
pub struct MemoryYrsRemoteLogStore {
    inner: Arc<Mutex<HashMap<ResourceId, (Vec<u8>, String)>>>,
}

impl MemoryYrsRemoteLogStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl YrsRemoteLogStore for MemoryYrsRemoteLogStore {
    fn put_log(
        &self,
        page_id: ResourceId,
        base_hash: [u8; 32],
        updates: &[&[u8]],
        if_match: Option<&str>,
    ) -> Result<RemotePutResult> {
        let sidecar_id = collab_log_resource_id(page_id);
        let payload = encode_remote_log(page_id, base_hash, updates);
        let hash = content_hash_hex(&payload)?;
        let mut guard = self.inner.lock().map_err(|_| Error::RemotePayload {
            message: "memory remote log store poisoned".into(),
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

    fn get_log(&self, page_id: ResourceId) -> Result<Option<RemoteLogPullResult>> {
        let sidecar_id = collab_log_resource_id(page_id);
        let guard = self.inner.lock().map_err(|_| Error::RemotePayload {
            message: "memory remote log store poisoned".into(),
        })?;
        let Some((payload, hash)) = guard.get(&sidecar_id) else {
            return Ok(None);
        };
        let decoded = decode_remote_log(page_id, payload)?;
        Ok(Some(RemoteLogPullResult {
            page_id,
            sidecar_id,
            base_hash: decoded.base_hash,
            updates: decoded.updates,
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
    fn log_encode_decode_roundtrip() {
        let page = ResourceId::new();
        let u1 = make_text_update("first");
        let u2 = make_text_update("second");
        let base = [7u8; 32];
        let payload = encode_remote_log(page, base, &[&u1, &u2]);
        let decoded = decode_remote_log(page, &payload).unwrap();
        assert_eq!(decoded.base_hash, base);
        assert_eq!(decoded.updates, vec![u1, u2]);
    }

    #[test]
    fn log_rejects_wrong_page_id() {
        let page = ResourceId::new();
        let other = ResourceId::new();
        let payload = encode_remote_log(page, REMOTE_LOG_UNKNOWN_BASE_HASH, &[&make_text_update("x")]);
        let err = decode_remote_log(other, &payload).unwrap_err();
        assert!(matches!(err, Error::RemotePayload { .. }));
    }

    #[test]
    fn snapshot_and_log_sidecar_ids_differ() {
        let page = ResourceId::new();
        let snapshot = collab_snapshot_resource_id(page);
        let log = collab_log_resource_id(page);
        assert_ne!(snapshot, log);
        assert_ne!(snapshot, page);
        assert_ne!(log, page);
    }

    #[test]
    fn append_then_decode_yields_both_updates() {
        let page = ResourceId::new();
        let u1 = make_text_update("one");
        let u2 = make_text_update("two");
        let first = append_update(page, &[], &u1).unwrap();
        let second = append_update(page, &first, &u2).unwrap();
        let decoded = decode_remote_log(page, &second).unwrap();
        assert_eq!(decoded.updates.len(), 2);
        assert_eq!(decoded.updates[0], u1);
        assert_eq!(decoded.updates[1], u2);
        assert_eq!(decoded.base_hash, REMOTE_LOG_UNKNOWN_BASE_HASH);
    }

    #[test]
    fn memory_log_store_rejects_stale_if_match() {
        let store = MemoryYrsRemoteLogStore::new();
        let page = ResourceId::new();
        let u1 = make_text_update("a");
        let first = store
            .put_log(page, REMOTE_LOG_UNKNOWN_BASE_HASH, &[&u1], None)
            .unwrap();
        let err = store
            .put_log(
                page,
                REMOTE_LOG_UNKNOWN_BASE_HASH,
                &[&make_text_update("b")],
                Some("deadbeef"),
            )
            .unwrap_err();
        assert!(matches!(err, Error::RemoteConflict { .. }));
        let ok = store
            .put_log(
                page,
                REMOTE_LOG_UNKNOWN_BASE_HASH,
                &[&make_text_update("b")],
                Some(&first.content_hash),
            )
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
