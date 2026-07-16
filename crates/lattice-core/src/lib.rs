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
mod home;
mod links;
mod manifest;
mod resource;
mod template;
mod validate;
mod watcher;
mod workspace;

pub use error::Error;
pub use home::{
    effective_default_workspace, ensure_lattice_home, lattice_home_path, LatticeHome,
    DEFAULT_WORKSPACE_NAME, LATTICE_HOME_NAME, SETTINGS_DIR_NAME, STATE_DIR_NAME,
    WORKSPACES_DIR_NAME,
};
pub use links::{
    parse_resource_links, MarkdownLinkKind, ParsedResourceLink, ResourceCatalog,
    ResourceLinkResolution, ResourceLinkTarget,
};
pub use manifest::{
    Capabilities, WorkspaceDefaults, WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME,
};
pub use resource::{Resource, ResourceKind};
pub use template::{
    apply_template, init_with_template, DefaultWorkspaceStatus, ProvisionDiagnostic,
    TemplateDescriptor, TemplateVisibility, WorkspaceCreationMode, WorkspaceCreationPlan,
    WorkspaceProvisionOutcome, WorkspaceProvisioner, WorkspaceTemplate,
};
pub use validate::{Diagnostic, Severity};
pub use watcher::{WorkspaceEvent, WorkspaceWatcher};
pub use workspace::{Workspace, OPERATIONAL_DIR};

pub type Result<T> = std::result::Result<T, Error>;
