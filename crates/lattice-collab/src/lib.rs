//! In-memory Yrs collaboration sessions for latticed (ADR 0055).
//!
//! Documents are keyed by stable [`ResourceId`] only — never by path. Path may
//! be used at open time to register/resolve a resource via LatticeFS, but the
//! session map always uses the registry UUID.
//!
//! Durable reopen uses an append-only update journal + snapshot under
//! `.lattice/collab/<uuid>/` (see [`journal`]).

mod error;
mod id;
mod journal;
mod registry;
mod session;

pub use error::{Error, Result};
pub use id::parse_doc_resource_id;
pub use journal::{
    journal_dir, journal_exists, COLLAB_SUBDIR, SNAPSHOT_FILENAME, UPDATES_FILENAME,
};
pub use registry::{CollabRegistry, CollabSnapshot, OpenedDoc};
pub use session::CollabSession;
