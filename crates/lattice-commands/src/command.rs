use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use lattice_data::{CellValue, FieldType, RollupAggregate};
use serde::{Deserialize, Serialize};

/// Owned column specification for [`Command::ColumnsAdd`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub name: String,
    #[serde(rename = "field-type")]
    pub field_type: FieldType,
    #[serde(
        rename = "relation-table",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub relation_table: Option<String>,
    #[serde(
        rename = "lookup-relation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub lookup_relation: Option<String>,
    #[serde(
        rename = "lookup-field",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub lookup_field: Option<String>,
    #[serde(
        rename = "rollup-relation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub rollup_relation: Option<String>,
    #[serde(
        rename = "rollup-aggregate",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub rollup_aggregate: Option<RollupAggregate>,
    #[serde(
        rename = "rollup-field",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub rollup_field: Option<String>,
}

impl ColumnSpec {
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            relation_table: None,
            lookup_relation: None,
            lookup_field: None,
            rollup_relation: None,
            rollup_aggregate: None,
            rollup_field: None,
        }
    }

    pub fn relation(name: impl Into<String>, relation_table: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::Relation,
            relation_table: Some(relation_table.into()),
            lookup_relation: None,
            lookup_field: None,
            rollup_relation: None,
            rollup_aggregate: None,
            rollup_field: None,
        }
    }

    pub fn lookup(
        name: impl Into<String>,
        lookup_relation: impl Into<String>,
        lookup_field: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::Lookup,
            relation_table: None,
            lookup_relation: Some(lookup_relation.into()),
            lookup_field: Some(lookup_field.into()),
            rollup_relation: None,
            rollup_aggregate: None,
            rollup_field: None,
        }
    }

    pub fn rollup(
        name: impl Into<String>,
        rollup_relation: impl Into<String>,
        aggregate: RollupAggregate,
        rollup_field: Option<impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::Rollup,
            relation_table: None,
            lookup_relation: None,
            lookup_field: None,
            rollup_relation: Some(rollup_relation.into()),
            rollup_aggregate: Some(aggregate),
            rollup_field: rollup_field.map(Into::into),
        }
    }
}

/// A file node placement on a JSON Canvas. The canvas path and resource path
/// are workspace-relative; the command engine validates both before writing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasPlaceResource {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    #[serde(rename = "resource-path")]
    pub resource_path: PathBuf,
    #[serde(rename = "node-id")]
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One node's new position in a batched canvas move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasNodeMove {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

/// A batched move of existing JSON Canvas nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasMoveNodes {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    pub nodes: Vec<CanvasNodeMove>,
}

/// Removal of existing JSON Canvas nodes and their incident edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasRemoveNodes {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    #[serde(rename = "node-ids")]
    pub node_ids: Vec<String>,
}

/// Create a directed edge between two existing JSON Canvas nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasAddEdge {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    #[serde(rename = "edge-id")]
    pub edge_id: String,
    #[serde(rename = "from-node")]
    pub from_node: String,
    #[serde(rename = "to-node")]
    pub to_node: String,
    #[serde(rename = "from-side", default, skip_serializing_if = "Option::is_none")]
    pub from_side: Option<String>,
    #[serde(rename = "to-side", default, skip_serializing_if = "Option::is_none")]
    pub to_side: Option<String>,
}

/// One node's new size in a batched canvas resize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasNodeResize {
    pub id: String,
    pub width: f64,
    pub height: f64,
}

/// A batched resize of existing JSON Canvas nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasResizeNodes {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    pub nodes: Vec<CanvasNodeResize>,
}

/// Removal of existing JSON Canvas edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasRemoveEdges {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    #[serde(rename = "edge-ids")]
    pub edge_ids: Vec<String>,
}

/// Place a Markdown sticky / text node on a JSON Canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasAddTextNode {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    #[serde(rename = "node-id")]
    pub node_id: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Update the body of an existing JSON Canvas text node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasUpdateTextNode {
    pub path: PathBuf,
    #[serde(rename = "base-revision")]
    pub base_revision: String,
    #[serde(rename = "node-id")]
    pub node_id: String,
    pub text: String,
}

/// The v0 semantic command set.
///
/// These are whole-file (whole-resource) operations; block-level and
/// dataset-level commands come later (docs/17). Every mutation the product
/// performs — from the GUI, the CLI, or a future API/MCP client — is expressed
/// as one of these and flows through [`crate::CommandEngine`] (ADR 0007).
///
/// Serialization uses kebab-case type tags so the on-disk history JSON is
/// stable and human-legible:
///
/// ```json
/// { "type": "page-update", "path": "Notes/A.md", "content": "…", "base-revision": "sha256:…" }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Command {
    /// Create a page at `path` with `content`. Precondition: `path` is absent.
    PageCreate { path: PathBuf, content: String },

    /// Create an arbitrary file resource at `path`. Used for binary assets
    /// imported by editor paste/drop. Precondition: `path` is absent.
    ResourceCreate {
        path: PathBuf,
        #[serde(with = "base64_bytes")]
        content: Vec<u8>,
    },

    /// Replace the content of the page at `path`. Precondition: the on-disk
    /// revision equals `base_revision` (optimistic concurrency).
    PageUpdate {
        path: PathBuf,
        content: String,
        /// `"sha256:<hex>"` the update is based on.
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Replace an editable file resource with bytes. The command is the
    /// native foundation for text-oriented viewers; callers must provide the
    /// materialized revision they edited from. Binary profiles use dedicated
    /// format commands or remain read-only.
    ResourceUpdate {
        path: PathBuf,
        #[serde(with = "base64_bytes")]
        content: Vec<u8>,
        /// `"sha256:<hex>"` the update is based on.
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Replace `lattice.yaml` after validating a typed workspace-manifest
    /// update. The command stores serialized YAML for durable history while
    /// callers construct it from `lattice_core::WorkspaceManifest`.
    WorkspaceManifestUpdate {
        content: String,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Rename a resource. Precondition: `from` present, `to` absent.
    ResourceRename { from: PathBuf, to: PathBuf },

    /// Move a resource into an existing directory. Precondition: `from`
    /// present, `to_dir` is a directory, and `to_dir/<name>` is absent.
    ResourceMove {
        from: PathBuf,
        #[serde(rename = "to-dir")]
        to_dir: PathBuf,
    },

    /// Delete a resource (sent to the OS Trash; bytes captured in history for
    /// single files so undo can restore without touching the Trash).
    /// Precondition: `path` present.
    ResourceDelete { path: PathBuf },

    /// Create an empty folder at `path`. Precondition: `path` is absent and
    /// its parent directory exists (or `path` is top-level).
    FolderCreate { path: PathBuf },

    /// Create a `.data` package at `path`. Precondition: `path` is absent.
    TableCreate {
        path: PathBuf,
        title: String,
        #[serde(rename = "table-name")]
        table_name: String,
    },

    /// Add a table to an existing `.data` package. Precondition: package
    /// `database.sqlite` revision equals `base_revision` and the table is absent.
    TableAdd {
        path: PathBuf,
        #[serde(rename = "table-name")]
        table_name: String,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Drop a table from a `.data` package. Recorded as the inverse of
    /// [`Command::TableAdd`]; not applied as a forward user command.
    TableDrop {
        path: PathBuf,
        #[serde(rename = "table-name")]
        table_name: String,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Add columns to a table inside a `.data` package. Precondition: package
    /// revision equals `base_revision`. Existing column names are skipped.
    ColumnsAdd {
        path: PathBuf,
        table: String,
        columns: Vec<ColumnSpec>,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Drop columns from a table. Recorded as the inverse of
    /// [`Command::ColumnsAdd`]; not applied as a forward user command.
    ColumnsRemove {
        path: PathBuf,
        table: String,
        columns: Vec<String>,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Insert a row into a table inside a `.data` package. When `id` is set
    /// (recorded in history after the first apply), the row is restored with
    /// that id instead of generating a new one.
    RecordInsert {
        path: PathBuf,
        table: String,
        values: BTreeMap<String, CellValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },

    /// Update a row. Precondition: package `database.sqlite` revision equals
    /// `base_revision`.
    RecordUpdate {
        path: PathBuf,
        table: String,
        id: String,
        values: BTreeMap<String, CellValue>,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Delete a row. Precondition: package revision equals `base_revision`.
    RecordDelete {
        path: PathBuf,
        table: String,
        id: String,
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Write or replace `views/{view_name}.yaml` inside a `.data` package.
    ViewSave {
        path: PathBuf,
        #[serde(rename = "view-name")]
        view_name: String,
        content: String,
    },

    /// Write or replace `forms/{form_name}.form.yaml` inside a `.data` package.
    FormSave {
        path: PathBuf,
        #[serde(rename = "form-name")]
        form_name: String,
        content: String,
    },

    /// Place a file node into a JSON Canvas while preserving the rest of the
    /// original JSON value, including fields unknown to Lattice.
    CanvasPlaceResource {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        #[serde(rename = "resource-path")]
        resource_path: PathBuf,
        #[serde(rename = "node-id")]
        node_id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },

    /// Move one or more JSON Canvas nodes in one semantic transaction.
    CanvasMoveNodes {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        nodes: Vec<CanvasNodeMove>,
    },

    /// Remove nodes and their incident edges from a JSON Canvas.
    CanvasRemoveNodes {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        #[serde(rename = "node-ids")]
        node_ids: Vec<String>,
    },

    /// Connect two existing JSON Canvas nodes with a directed edge.
    CanvasAddEdge {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        #[serde(rename = "edge-id")]
        edge_id: String,
        #[serde(rename = "from-node")]
        from_node: String,
        #[serde(rename = "to-node")]
        to_node: String,
        #[serde(rename = "from-side", default, skip_serializing_if = "Option::is_none")]
        from_side: Option<String>,
        #[serde(rename = "to-side", default, skip_serializing_if = "Option::is_none")]
        to_side: Option<String>,
    },

    /// Resize one or more JSON Canvas nodes in one semantic transaction.
    CanvasResizeNodes {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        nodes: Vec<CanvasNodeResize>,
    },

    /// Remove edges from a JSON Canvas.
    CanvasRemoveEdges {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        #[serde(rename = "edge-ids")]
        edge_ids: Vec<String>,
    },

    /// Add a sticky / text node to a JSON Canvas.
    CanvasAddTextNode {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        #[serde(rename = "node-id")]
        node_id: String,
        text: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },

    /// Update the text of an existing sticky / text node.
    CanvasUpdateTextNode {
        path: PathBuf,
        #[serde(rename = "base-revision")]
        base_revision: String,
        #[serde(rename = "node-id")]
        node_id: String,
        text: String,
    },
}

mod base64_bytes {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
    }
}

impl Command {
    /// The path whose post-apply state the recorded `resulting_revision`
    /// describes — used as the target of the external-write undo guard.
    ///
    /// For creates/updates it is the written path; for a rename/move it is the
    /// destination; for a delete it is the (now-absent) path.
    pub(crate) fn guard_path(&self) -> PathBuf {
        match self {
            Command::PageCreate { path, .. } => path.clone(),
            Command::ResourceCreate { path, .. } => path.clone(),
            Command::PageUpdate { path, .. } => path.clone(),
            Command::ResourceUpdate { path, .. } => path.clone(),
            Command::WorkspaceManifestUpdate { .. } => {
                PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME)
            }
            Command::ResourceRename { to, .. } => to.clone(),
            Command::ResourceMove { from, to_dir } => to_dir.join(file_name(from)),
            Command::ResourceDelete { path } => path.clone(),
            Command::FolderCreate { path } => path.clone(),
            Command::TableCreate { path, .. }
            | Command::TableAdd { path, .. }
            | Command::TableDrop { path, .. }
            | Command::ColumnsAdd { path, .. }
            | Command::ColumnsRemove { path, .. } => path.clone(),
            Command::RecordInsert { path, .. }
            | Command::RecordUpdate { path, .. }
            | Command::RecordDelete { path, .. } => path.clone(),
            Command::ViewSave {
                path, view_name, ..
            } => view_file_path(path, view_name),
            Command::FormSave {
                path, form_name, ..
            } => form_file_path(path, form_name),
            Command::CanvasPlaceResource { path, .. }
            | Command::CanvasMoveNodes { path, .. }
            | Command::CanvasRemoveNodes { path, .. }
            | Command::CanvasAddEdge { path, .. }
            | Command::CanvasResizeNodes { path, .. }
            | Command::CanvasRemoveEdges { path, .. }
            | Command::CanvasAddTextNode { path, .. }
            | Command::CanvasUpdateTextNode { path, .. } => path.clone(),
        }
    }

    /// Every path this command reads or writes, for intra-transaction conflict
    /// detection (v0 rejects transactions that touch a path more than once).
    pub(crate) fn touched_paths(&self) -> Vec<PathBuf> {
        match self {
            Command::PageCreate { path, .. } => vec![path.clone()],
            Command::ResourceCreate { path, .. } => vec![path.clone()],
            Command::PageUpdate { path, .. } => vec![path.clone()],
            Command::ResourceUpdate { path, .. } => vec![path.clone()],
            Command::WorkspaceManifestUpdate { .. } => {
                vec![PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME)]
            }
            Command::ResourceRename { from, to } => vec![from.clone(), to.clone()],
            Command::ResourceMove { from, to_dir } => {
                vec![from.clone(), to_dir.join(file_name(from))]
            }
            Command::ResourceDelete { path } => vec![path.clone()],
            Command::FolderCreate { path } => vec![path.clone()],
            Command::TableCreate { path, .. }
            | Command::TableAdd { path, .. }
            | Command::TableDrop { path, .. }
            | Command::ColumnsAdd { path, .. }
            | Command::ColumnsRemove { path, .. } => vec![path.clone()],
            Command::RecordInsert { path, .. }
            | Command::RecordUpdate { path, .. }
            | Command::RecordDelete { path, .. } => vec![path.clone()],
            Command::ViewSave {
                path, view_name, ..
            } => vec![view_file_path(path, view_name)],
            Command::FormSave {
                path, form_name, ..
            } => vec![form_file_path(path, form_name)],
            Command::CanvasPlaceResource { path, .. }
            | Command::CanvasMoveNodes { path, .. }
            | Command::CanvasRemoveNodes { path, .. }
            | Command::CanvasAddEdge { path, .. }
            | Command::CanvasResizeNodes { path, .. }
            | Command::CanvasRemoveEdges { path, .. }
            | Command::CanvasAddTextNode { path, .. }
            | Command::CanvasUpdateTextNode { path, .. } => vec![path.clone()],
        }
    }
}

/// Workspace-relative path to a view YAML inside a `.data` package.
pub(crate) fn view_file_path(package: &std::path::Path, view_name: &str) -> PathBuf {
    package.join("views").join(format!("{view_name}.yaml"))
}

/// Workspace-relative path to a form YAML inside a `.data` package.
pub(crate) fn form_file_path(package: &std::path::Path, form_name: &str) -> PathBuf {
    package
        .join("forms")
        .join(format!("{form_name}{}", lattice_data::FORM_FILE_SUFFIX))
}

/// The final path component of `path`, or the whole path if it has none.
pub(crate) fn file_name(path: &std::path::Path) -> PathBuf {
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

/// An atomic unit of intent: a set of commands applied all-or-nothing, with a
/// human-readable summary and optional idempotency key.
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Assigned by [`Transaction::new`]; a UUID v7 (time-ordered).
    pub id: String,
    /// Human-readable one-line description of the change.
    pub summary: String,
    pub commands: Vec<Command>,
    /// Replaying a transaction whose key already exists is a no-op that returns
    /// the original receipt.
    pub idempotency_key: Option<String>,
}

impl Transaction {
    /// Build a transaction with a fresh time-ordered id.
    pub fn new(summary: impl Into<String>, commands: Vec<Command>) -> Self {
        Transaction {
            id: uuid::Uuid::now_v7().to_string(),
            summary: summary.into(),
            commands,
            idempotency_key: None,
        }
    }

    /// Attach an idempotency key.
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

/// The outcome of one applied command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    /// The resulting content revision, when the command produces one (creates,
    /// updates, and the destination of a rename/move). `None` for deletes.
    pub resulting_revision: Option<String>,
}

/// The result of applying a transaction.
#[derive(Debug, Clone)]
pub struct TransactionReceipt {
    pub transaction_id: String,
    pub summary: String,
    /// One entry per command, in order.
    pub outcomes: Vec<CommandOutcome>,
    /// True when this receipt was replayed from history because the
    /// idempotency key already existed (no mutation occurred).
    pub idempotent_replay: bool,
}

/// A path that changed as a result of undo/redo (shell tabs/selection follow these).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRemap {
    /// Path before the undo/redo was applied (where open tabs may still point).
    pub from: PathBuf,
    /// Path after the undo/redo (restored or re-applied destination).
    pub to: PathBuf,
}

/// The result of an undo or redo.
#[derive(Debug, Clone)]
pub struct UndoReport {
    pub transaction_id: String,
    pub summary: String,
    /// Rename/move path pairs implied by applied inverse (undo) or forward (redo)
    /// commands. Empty when the transaction did not relocate resources.
    pub path_remaps: Vec<PathRemap>,
}

/// Extract rename/move remaps from commands that are about to run (inverses for
/// undo, forwards for redo). `ResourceMove` is included for completeness even
/// though move inverses are recorded as `ResourceRename` today.
pub fn path_remaps_from_commands(commands: &[Command]) -> Vec<PathRemap> {
    commands
        .iter()
        .filter_map(|command| match command {
            Command::ResourceRename { from, to } => Some(PathRemap {
                from: from.clone(),
                to: to.clone(),
            }),
            Command::ResourceMove { from, to_dir } => {
                let dest = to_dir.join(file_name(from));
                Some(PathRemap {
                    from: from.clone(),
                    to: dest,
                })
            }
            _ => None,
        })
        .collect()
}

/// One row in the transaction history listing.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub summary: String,
    pub created_at: SystemTime,
    pub idempotency_key: Option<String>,
    pub undone: bool,
    pub command_count: usize,
}
