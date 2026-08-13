//! In-memory Yrs collaboration sessions for latticed (ADR 0055).
//!
//! Documents are keyed by stable [`ResourceId`] only — never by path. Path may
//! be used at open time to register/resolve a resource via LatticeFS, but the
//! session map always uses the registry UUID.
//!
//! Durable reopen uses an append-only update journal + snapshot under
//! `.lattice/collab/<uuid>/` (see [`journal`]).
//!
//! Optional remote exchange (S8) stores opaque Yrs snapshots via
//! [`remote::YrsRemoteStore`] on a sidecar ResourceId — see
//! `docs/dev/yrs-remote-provider.md`.

mod error;
mod id;
mod journal;
mod registry;
mod remote;
mod session;

pub use error::{Error, Result};
pub use id::parse_doc_resource_id;
pub use journal::{
    journal_dir, journal_exists, COLLAB_SUBDIR, SNAPSHOT_FILENAME, UPDATES_FILENAME,
};
pub use registry::{CollabRegistry, CollabSnapshot, OpenedDoc};
pub use remote::{
    append_update, collab_log_resource_id, collab_snapshot_resource_id, decode_remote_log,
    decode_remote_snapshot, encode_remote_log, encode_remote_snapshot, exchange_snapshot_roundtrip,
    MemoryYrsRemoteLogStore, MemoryYrsRemoteStore, RemoteLogDecoded, RemoteLogPullResult,
    RemotePullResult, RemotePutResult, YrsRemoteLogStore, YrsRemoteStore, REMOTE_LOG_MAGIC,
    REMOTE_LOG_MAX_BYTES, REMOTE_LOG_MAX_UPDATES, REMOTE_LOG_UNKNOWN_BASE_HASH,
    REMOTE_LOG_VERSION, REMOTE_SNAPSHOT_MAGIC, REMOTE_SNAPSHOT_VERSION,
};
pub use session::CollabSession;
