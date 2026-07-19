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
mod link_repair;
mod links;
mod manifest;
mod resource;
mod resource_runtime;
mod template;
mod validate;
mod watcher;
mod workspace;

pub use error::Error;
pub use home::{
    default_debug_home_path, effective_default_workspace, ensure_lattice_home,
    initialize_active_lattice_home, initialize_dev_lattice_home, initialize_lattice_home,
    lattice_dev_home_enabled, lattice_force_prod_home_enabled, lattice_home_path, DEV_TEMPLATE_ID,
    DEV_WORKSPACE_NAME, LatticeHome, DEFAULT_DEBUG_HOME_RELATIVE, DEFAULT_WORKSPACE_NAME,
    LATTICE_DEV_HOME_ENV, LATTICE_FORCE_PROD_HOME_ENV, LATTICE_HOME_ENV, LATTICE_HOME_NAME,
    SETTINGS_DIR_NAME, STATE_DIR_NAME, WORKSPACES_DIR_NAME,
};
pub use link_repair::{
    apply_span_replacements, build_link_repair_plan, build_repair_candidate, format_link_text,
    merge_batch_link_repair_plans, path_is_co_moved, resolution_targets_path, rewrite_link_target,
    BatchLinkRepairPlan, LinkOccurrence, LinkRepairCandidate, LinkRepairPathChange, LinkRepairPlan,
    LinkRepairProposalSummary, LinkRepairSource, LinkRepairStatus,
    LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP, LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD,
};
pub use links::{
    parse_resource_links, MarkdownLinkKind, ParsedResourceLink, ResourceCatalog,
    ResourceLinkResolution, ResourceLinkTarget,
};
pub use manifest::{
    Capabilities, DirectoryMeta, WorkspaceDefaults, WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME,
};
pub use resource::{Resource, ResourceKind};
pub use resource_runtime::{
    inspect_resource, read_resource_range, read_text_window, BuiltinFormatRegistry,
    FormatCapabilities, ResourceDiagnostic, ResourceEncoding, ResourceFormatProfile,
    ResourceInspection, ResourceRange, ResourceRuntimeError, TextWindow,
    DEFAULT_RESOURCE_EDIT_BYTES, MAX_FORMAT_PROBE_BYTES, MAX_RESOURCE_RANGE_BYTES,
};
pub use template::{
    apply_template, init_with_template, resolve_template_id, template_catalog,
    template_catalog_ids, template_descriptor,
    DefaultWorkspaceStatus, ProvisionDiagnostic, TemplateDescriptor, TemplateDirectory,
    TemplateVisibility, WorkspaceCreationMode, WorkspaceCreationPlan, WorkspaceProvisionOutcome,
    WorkspaceProvisioner, WorkspaceTemplate,
};
pub use validate::{Diagnostic, Severity};
pub use watcher::{
    WorkspaceEvent, WorkspaceWatcher, DEFAULT_DEBOUNCE_TIMEOUT, TEST_DEBOUNCE_TIMEOUT,
};
pub use workspace::{Workspace, OPERATIONAL_DIR};

pub type Result<T> = std::result::Result<T, Error>;
