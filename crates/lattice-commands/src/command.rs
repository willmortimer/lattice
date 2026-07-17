use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use lattice_data::CellValue;
use serde::{Deserialize, Serialize};

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

    /// Create a `.data` package at `path`. Precondition: `path` is absent.
    TableCreate {
        path: PathBuf,
        title: String,
        #[serde(rename = "table-name")]
        table_name: String,
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
            Command::TableCreate { path, .. } => path.clone(),
            Command::RecordInsert { path, .. }
            | Command::RecordUpdate { path, .. }
            | Command::RecordDelete { path, .. } => path.clone(),
            Command::ViewSave {
                path, view_name, ..
            } => view_file_path(path, view_name),
            Command::CanvasPlaceResource { path, .. }
            | Command::CanvasMoveNodes { path, .. }
            | Command::CanvasRemoveNodes { path, .. } => path.clone(),
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
            Command::TableCreate { path, .. } => vec![path.clone()],
            Command::RecordInsert { path, .. }
            | Command::RecordUpdate { path, .. }
            | Command::RecordDelete { path, .. } => vec![path.clone()],
            Command::ViewSave {
                path, view_name, ..
            } => vec![view_file_path(path, view_name)],
            Command::CanvasPlaceResource { path, .. }
            | Command::CanvasMoveNodes { path, .. }
            | Command::CanvasRemoveNodes { path, .. } => vec![path.clone()],
        }
    }
}

/// Workspace-relative path to a view YAML inside a `.data` package.
pub(crate) fn view_file_path(package: &std::path::Path, view_name: &str) -> PathBuf {
    package.join("views").join(format!("{view_name}.yaml"))
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

/// The result of an undo or redo.
#[derive(Debug, Clone)]
pub struct UndoReport {
    pub transaction_id: String,
    pub summary: String,
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
