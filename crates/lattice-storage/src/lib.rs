//! Durable storage foundation for Lattice.
//!
//! Every canonical mutation flows through this crate, so its contract is
//! correctness first: writes are materialized atomically (temp file in the
//! same directory, flushed, then renamed over the target), and each write is
//! recorded in a crash-recovery journal *before* it touches the filesystem.
//!
//! The pieces (see `docs/05-storage-filesystem-and-recovery.md`):
//!
//! - [`WorkspaceStore`] is the path/revision/atomicity abstraction. Two
//!   providers ship here: [`NativeWorkspaceStore`] over the real filesystem
//!   and [`MemoryWorkspaceStore`] for tests and previews.
//! - [`RecoveryJournal`] is the `.lattice/recovery.sqlite` write-ahead record
//!   of intent-to-write. Entries begun but never completed are crash evidence.
//! - [`BufferedWriter`] is the one true write path, tying the two together:
//!   journal append -> atomic materialize -> journal complete.
//!
//! Paths handed to a store are always workspace-relative; components that
//! escape the root (`..`, absolute paths) are rejected rather than followed.

mod error;
mod journal;
mod revision;
mod store;
mod writer;

pub use error::Error;
pub use journal::{PendingWrite, RecoveryJournal};
pub use revision::ResourceRevision;
pub use store::{
    MemoryWorkspaceStore, NativeWorkspaceStore, ResourceEntry, ResourceMetadata, WorkspaceStore,
};
pub use writer::BufferedWriter;

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
