//! In-memory Yrs collaboration sessions for latticed (ADR 0055 / Y0 pilot).
//!
//! Documents are keyed by stable [`ResourceId`] only — never by path. Path may
//! be used at open time to register/resolve a resource via LatticeFS, but the
//! session map always uses the registry UUID.
//!
//! Persistence under `.lattice/collab/` is intentionally out of scope (Y2).

mod error;
mod id;
mod registry;
mod session;

pub use error::{Error, Result};
pub use id::parse_doc_resource_id;
pub use registry::{CollabRegistry, CollabSnapshot, OpenedDoc};
pub use session::CollabSession;
