//! Core workspace model for Lattice.
//!
//! A Lattice workspace is an ordinary directory whose root contains a
//! `lattice.yaml` manifest. Canonical content is plain files; the hidden
//! `.lattice/` directory holds only derived or operational state.
//!
//! This crate provides workspace discovery, manifest parsing, resource
//! classification, and validation. It is headless by design: the CLI,
//! desktop shell, daemon, and server all compose it.

mod error;
mod manifest;
mod resource;
mod validate;
mod watcher;
mod workspace;

pub use error::Error;
pub use manifest::{Capabilities, WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME};
pub use resource::{Resource, ResourceKind};
pub use validate::{Diagnostic, Severity};
pub use watcher::{WorkspaceEvent, WorkspaceWatcher};
pub use workspace::{Workspace, OPERATIONAL_DIR};

pub type Result<T> = std::result::Result<T, Error>;
