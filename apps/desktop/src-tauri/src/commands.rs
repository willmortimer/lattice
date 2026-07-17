use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lattice_commands::{
    Command as SemanticCommand, CommandEngine, Error as CommandError, Transaction,
};
use lattice_core::{
    ensure_lattice_home, initialize_lattice_home, inspect_resource as inspect_native_resource,
    read_resource_range as read_native_resource_range, read_text_window as read_native_text_window,
    DefaultWorkspaceStatus, ProvisionDiagnostic, Resource, ResourceRuntimeError,
    TemplateDescriptor, Workspace, WorkspaceCreationMode, WorkspaceCreationPlan, WorkspaceDefaults,
    WorkspaceProvisioner,
};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request, Response};

const MAX_EDITOR_ASSET_BYTES: usize = 8 * 1024 * 1024;

/// Everything the frontend needs to render a workspace: its identity plus
/// the flat resource listing from [`Workspace::scan`].
#[derive(Debug, Serialize)]
pub struct WorkspaceSnapshot {
    pub root: String,
    pub title: String,
    pub id: String,
    pub resources: Vec<Resource>,
    pub capabilities: Vec<String>,
    pub defaults: WorkspaceDefaults,
    pub manifest_revision: String,
}

#[tauri::command]
pub fn open_workspace(path: String) -> Result<WorkspaceSnapshot, String> {
    let root = PathBuf::from(path);
    let workspace = Workspace::open(&root).map_err(|err| err.to_string())?;
    let resources = workspace.scan().map_err(|err| err.to_string())?;

    snapshot_from_parts(&workspace, resources)
}

/// Re-scan a workspace's resource listing without re-reading its manifest.
/// Lighter than [`open_workspace`] for refreshing the sidebar after a
/// `workspace-changed` event.
#[tauri::command]
pub fn list_resources(root: String) -> Result<Vec<Resource>, String> {
    let workspace = Workspace::open(Path::new(&root)).map_err(|err| err.to_string())?;
    workspace.scan().map_err(|err| err.to_string())
}

/// Canonicalize `root` and a `rel_path` candidate beneath it, rejecting `..`
/// traversal and absolute-path escapes (including through symlinks) by
/// requiring the resolved candidate to remain under the canonical root.
/// Returns `(canonical_root, canonical_candidate)`.
pub(crate) fn resolve_within_root(
    root: &str,
    rel_path: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let canonical_root = PathBuf::from(root)
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))?;

    let candidate = canonical_root.join(rel_path);
    let canonical_candidate = candidate
        .canonicalize()
        .map_err(|err| format!("cannot resolve {rel_path:?}: {err}"))?;

    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }

    Ok((canonical_root, canonical_candidate))
}

/// Read a text resource by path relative to `root`.
///
/// `root` and the resolved candidate path are both canonicalized and the
/// candidate is required to remain under the canonical root, which rejects
/// `..` traversal and absolute-path escapes (including through symlinks).
#[tauri::command]
pub fn read_file(root: String, rel_path: String) -> Result<String, String> {
    let (_, canonical_candidate) = resolve_within_root(&root, &rel_path)?;
    std::fs::read_to_string(&canonical_candidate).map_err(|err| err.to_string())
}

/// Read a binary resource through the same workspace containment check used
/// by text reads. The desktop frontend turns the returned bytes into a
/// short-lived Blob URL instead of exposing a globally scoped filesystem
/// asset protocol to the webview.
#[tauri::command]
pub fn read_binary_file(root: String, rel_path: String) -> Result<Vec<u8>, String> {
    let (_, canonical_candidate) = resolve_within_root(&root, &rel_path)?;
    std::fs::read(&canonical_candidate).map_err(|err| err.to_string())
}

/// Inspect a native resource without mutating it. Format recognition is
/// extension- and bounded-probe-based; the returned diagnostics are safe to
/// display even when a resource is malformed or only partially supported.
#[tauri::command]
pub fn inspect_resource(
    root: String,
    rel_path: String,
) -> Result<lattice_core::ResourceInspection, ResourceRuntimeError> {
    inspect_native_resource(Path::new(&root), Path::new(&rel_path))
}

/// Read a bounded raw byte range. Tauri's raw response avoids turning binary
/// content into a JSON array; callers obtain format and size metadata from
/// [`inspect_resource`].
#[tauri::command]
pub fn read_resource_range(
    root: String,
    rel_path: String,
    offset: u64,
    length: u64,
) -> Result<Response, ResourceRuntimeError> {
    let range = read_native_resource_range(Path::new(&root), Path::new(&rel_path), offset, length)?;
    Ok(Response::new(range.bytes))
}

/// Read a bounded, encoding-aware text window with a structured serde result.
#[tauri::command]
pub fn read_text_window(
    root: String,
    rel_path: String,
    offset: u64,
    length: u64,
) -> Result<lattice_core::TextWindow, ResourceRuntimeError> {
    read_native_text_window(Path::new(&root), Path::new(&rel_path), offset, length)
}

/// A page's content plus the content-hash revision it was read at, so the
/// editor can round-trip that revision back as `apply_page_update`'s
/// `base_revision` (optimistic concurrency, ADR 0007).
#[derive(Debug, Serialize)]
pub struct PageContent {
    pub content: String,
    pub revision: String,
}

/// Read a page and the revision it was read at, in one round trip.
#[tauri::command]
pub fn read_page(root: String, rel_path: String) -> Result<PageContent, String> {
    let (canonical_root, canonical_candidate) = resolve_within_root(&root, &rel_path)?;
    let content = std::fs::read_to_string(&canonical_candidate).map_err(|err| err.to_string())?;

    let store = NativeWorkspaceStore::new(&canonical_root);
    let revision = store
        .metadata(Path::new(&rel_path))
        .map_err(|err| err.to_string())?
        .revision
        .hash;

    Ok(PageContent { content, revision })
}

/// Errors returned by [`apply_page_update`] are plain strings (Tauri's IPC
/// error channel), but a stale base revision is a distinct, expected case
/// the frontend must react to (show the conflict banner) rather than a
/// generic failure — so it's marked with a `STALE_REVISION:` prefix the
/// frontend can detect without parsing prose.
pub(crate) const STALE_REVISION_PREFIX: &str = "STALE_REVISION:";

pub(crate) fn command_error_to_string(err: CommandError) -> String {
    match err {
        CommandError::StaleBaseRevision {
            path,
            expected,
            found,
        } => {
            format!(
                "{STALE_REVISION_PREFIX}{}|expected={expected}|found={found}",
                path.display()
            )
        }
        other => other.to_string(),
    }
}

/// Apply a `PageUpdate` command through the [`CommandEngine`]: replace the
/// page at `rel_path` with `content` if the on-disk revision still matches
/// `base_revision`. Returns the resulting revision on success.
///
/// On a stale base revision (the page changed on disk since the editor read
/// it), the error string is prefixed with `STALE_REVISION:` so the frontend
/// can show a conflict banner instead of a generic error.
#[tauri::command]
pub fn apply_page_update(
    root: String,
    rel_path: String,
    content: String,
    base_revision: String,
) -> Result<String, String> {
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    let receipt = engine
        .apply(Transaction::new(
            format!("Update page {rel_path}"),
            vec![SemanticCommand::PageUpdate {
                path: PathBuf::from(&rel_path),
                content,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;

    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "page update did not produce a resulting revision".to_string())
}

/// Apply a bounded byte-oriented resource edit through the semantic command
/// core. Binary payloads arrive as raw Tauri request bytes, so they never
/// become JSON byte arrays on the command boundary.
#[tauri::command]
pub fn apply_resource_update(request: Request<'_>) -> Result<String, String> {
    let content = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(_) => return Err("resource update requires a raw binary body".to_string()),
    };
    let root = request_header(&request, "x-lattice-root")?;
    let rel_path = request_header(&request, "x-lattice-path")?;
    let base_revision = request_header(&request, "x-lattice-base-revision")?;
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let receipt = engine
        .apply(Transaction::new(
            format!("Update resource {rel_path}"),
            vec![SemanticCommand::ResourceUpdate {
                path: PathBuf::from(&rel_path),
                content,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;
    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "resource update did not produce a resulting revision".to_string())
}

/// Create a new page at `rel_path`.
///
/// When `template_path` is set, Rust reads that workspace-relative Markdown
/// file, substitutes `{{title}}` / `{{date}}`, and writes the result through
/// the semantic command core — the frontend never supplies the template body.
/// `content` is used only for blank creates (`template_path` absent).
#[tauri::command]
pub fn create_page(
    root: String,
    rel_path: String,
    content: String,
    template_path: Option<String>,
    title: Option<String>,
) -> Result<String, String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;

    let receipt = engine
        .create_page(
            PathBuf::from(&rel_path),
            content,
            template_path.map(PathBuf::from),
            title,
        )
        .map_err(command_error_to_string)?;

    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "page create did not produce a resulting revision".to_string())
}

/// Import a pasted or dropped editor asset beside its containing page.
///
/// Assets are stored in an `assets/` directory relative to the page, receive
/// a collision-free filename, and are created through the semantic command
/// engine. The returned path is page-relative for direct insertion into
/// Markdown.
fn decode_header(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| "invalid percent-encoded asset metadata".to_string())?;
            let pair = std::str::from_utf8(hex)
                .map_err(|_| "invalid percent-encoded asset metadata".to_string())?;
            decoded.push(
                u8::from_str_radix(pair, 16)
                    .map_err(|_| "invalid percent-encoded asset metadata".to_string())?,
            );
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "asset metadata is not valid UTF-8".to_string())
}

fn request_header(request: &Request<'_>, name: &str) -> Result<String, String> {
    let value = request
        .headers()
        .get(name)
        .ok_or_else(|| format!("missing {name} header"))?
        .to_str()
        .map_err(|_| format!("{name} header is invalid"))?;
    decode_header(value)
}

#[tauri::command]
pub fn create_asset(request: Request<'_>) -> Result<String, String> {
    let content = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(_) => return Err("asset import requires a raw binary body".to_string()),
    };
    create_asset_inner(
        request_header(&request, "x-lattice-root")?,
        request_header(&request, "x-lattice-page-path")?,
        request_header(&request, "x-lattice-file-name")?,
        content,
    )
}

fn create_asset_inner(
    root: String,
    page_path: String,
    file_name: String,
    content: Vec<u8>,
) -> Result<String, String> {
    if content.is_empty() {
        return Err("cannot import an empty asset".to_string());
    }
    if content.len() > MAX_EDITOR_ASSET_BYTES {
        return Err(format!(
            "editor assets are limited to {} MiB",
            MAX_EDITOR_ASSET_BYTES / (1024 * 1024)
        ));
    }

    let (canonical_root, _) = resolve_within_root(&root, &page_path)?;
    let safe_name = Path::new(&file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| "asset filename is invalid".to_string())?;
    if safe_name != file_name {
        return Err("asset filename must not contain path separators".to_string());
    }

    let page = Path::new(&page_path);
    let page_dir = page.parent().unwrap_or_else(|| Path::new(""));
    let asset_dir = page_dir.join("assets");
    let file_path = Path::new(safe_name);
    let stem = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let extension = file_path.extension().and_then(|value| value.to_str());

    let store = NativeWorkspaceStore::new(&canonical_root);
    let mut candidate = asset_dir.join(safe_name);
    let mut suffix = 2usize;
    while store.metadata(&candidate).is_ok() {
        let next_name = match extension {
            Some(extension) => format!("{stem} {suffix}.{extension}"),
            None => format!("{stem} {suffix}"),
        };
        candidate = asset_dir.join(next_name);
        suffix += 1;
    }

    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let receipt = engine
        .apply(Transaction::new(
            format!("Import asset {}", candidate.display()),
            vec![SemanticCommand::ResourceCreate {
                path: candidate.clone(),
                content,
            }],
        ))
        .map_err(command_error_to_string)?;

    if receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.as_ref())
        .is_none()
    {
        return Err("asset import did not produce a resulting revision".to_string());
    }

    let relative_to_page = candidate
        .strip_prefix(page_dir)
        .unwrap_or(&candidate)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(relative_to_page)
}

/// Rename a resource through the semantic command core.
#[tauri::command]
pub fn rename_resource(root: String, from: String, to: String) -> Result<(), String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            format!("Rename {from} to {to}"),
            vec![SemanticCommand::ResourceRename {
                from: PathBuf::from(from),
                to: PathBuf::from(to),
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub summary: String,
    pub created_at: u64,
    pub undone: bool,
    pub command_count: usize,
}

/// A bounded history listing for the resource inspector.
#[tauri::command]
pub fn list_history(root: String, limit: usize) -> Result<Vec<HistoryItem>, String> {
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    let entries = engine
        .history(limit.min(100))
        .map_err(command_error_to_string)?;
    Ok(entries
        .into_iter()
        .map(|entry| HistoryItem {
            id: entry.id,
            summary: entry.summary,
            created_at: entry
                .created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            undone: entry.undone,
            command_count: entry.command_count,
        })
        .collect())
}

/// Undo the most recent transaction recorded in this workspace's history,
/// if any. Used by the command palette's "Undo" action.
///
/// Returns the summary of the transaction that was undone, or `None` if
/// there was nothing left to undo.
#[tauri::command]
pub fn undo_last(root: String) -> Result<Option<String>, String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    let report = engine.undo().map_err(command_error_to_string)?;
    Ok(report.map(|r| r.summary))
}

/// Snapshot of `~/Lattice` after ensuring the layout exists.
#[derive(Debug, Serialize)]
pub struct LatticeHomeInfo {
    pub root: String,
    pub workspaces: String,
    pub settings: String,
    pub default_workspace: Option<WorkspaceSnapshot>,
    pub diagnostics: Vec<ProvisionDiagnostic>,
}

fn snapshot_from_workspace(workspace: &Workspace) -> Result<WorkspaceSnapshot, String> {
    let resources = workspace.scan().map_err(|err| err.to_string())?;
    snapshot_from_parts(workspace, resources)
}

fn snapshot_from_parts(
    workspace: &Workspace,
    resources: Vec<Resource>,
) -> Result<WorkspaceSnapshot, String> {
    let manifest = workspace.manifest();
    let store = NativeWorkspaceStore::new(workspace.root());
    let manifest_revision = store
        .metadata(Path::new(lattice_core::WORKSPACE_MANIFEST_FILENAME))
        .map_err(|error| error.to_string())?
        .revision
        .hash;
    Ok(WorkspaceSnapshot {
        root: workspace.root().to_string_lossy().into_owned(),
        title: manifest.title.clone(),
        id: manifest.id.clone(),
        resources,
        capabilities: manifest.capabilities.enabled.clone(),
        defaults: manifest.defaults.clone(),
        manifest_revision,
    })
}

/// Explicitly initialize Lattice home, provisioning a Personal workspace only
/// when no valid workspace exists. Ordinary startup never calls this command.
#[tauri::command]
pub fn ensure_home() -> Result<LatticeHomeInfo, String> {
    let (home, outcome) = initialize_lattice_home().map_err(|err| err.to_string())?;
    let default_workspace = Some(snapshot_from_workspace(&outcome.workspace)?);
    Ok(LatticeHomeInfo {
        root: home.root.to_string_lossy().into_owned(),
        workspaces: home.workspaces.to_string_lossy().into_owned(),
        settings: home.settings.to_string_lossy().into_owned(),
        default_workspace,
        diagnostics: outcome.diagnostics,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProvisionResult {
    pub workspace: WorkspaceSnapshot,
    pub default_workspace_status: DefaultWorkspaceStatus,
    pub diagnostics: Vec<ProvisionDiagnostic>,
}

#[tauri::command]
pub fn create_workspace(
    path: String,
    title: Option<String>,
    template: String,
    set_default: bool,
    initialize_existing: bool,
) -> Result<WorkspaceProvisionResult, String> {
    let root = PathBuf::from(&path);
    let title = title.unwrap_or_else(|| {
        root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string()
    });
    let mut outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
        target: root,
        title,
        template_id: template,
        mode: if initialize_existing {
            WorkspaceCreationMode::ExistingDirectory
        } else {
            WorkspaceCreationMode::NewDirectory
        },
    })
    .map_err(|err| err.to_string())?;
    if set_default {
        match ensure_lattice_home()
            .map_err(|error| error.to_string())
            .and_then(|home| {
                home.set_default_workspace(outcome.workspace.root())
                    .map_err(|error| error.to_string())
            }) {
            Ok(_) => outcome.default_workspace_status = DefaultWorkspaceStatus::Updated,
            Err(error) => {
                outcome.default_workspace_status = DefaultWorkspaceStatus::Failed;
                outcome.diagnostics.push(ProvisionDiagnostic {
                    code: "default-workspace-save-failed".into(),
                    message: format!(
                        "The workspace was created, but Lattice could not make it the default: {error}"
                    ),
                    retryable: true,
                });
            }
        }
    }
    Ok(WorkspaceProvisionResult {
        workspace: snapshot_from_workspace(&outcome.workspace)?,
        default_workspace_status: outcome.default_workspace_status,
        diagnostics: outcome.diagnostics,
    })
}

/// Built-in workspace templates for the New Workspace gallery.
#[tauri::command]
pub fn list_templates() -> Vec<TemplateDescriptor> {
    lattice_core::WorkspaceTemplate::gallery()
}

#[tauri::command]
pub fn update_workspace_manifest(
    root: String,
    enabled_capabilities: Vec<String>,
    quick_note_directory: String,
    base_revision: String,
) -> Result<WorkspaceSnapshot, String> {
    if quick_note_directory.trim().is_empty() || quick_note_directory.contains('\\') {
        return Err("Quick Note directory must be a non-empty workspace-relative path.".into());
    }
    let quick_note_path = Path::new(&quick_note_directory);
    if quick_note_path.is_absolute()
        || quick_note_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir
                    | std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("Quick Note directory must be workspace-relative.".into());
    }
    let workspace = Workspace::open(Path::new(&root)).map_err(|error| error.to_string())?;
    let mut manifest = workspace.manifest().clone();
    const MUTABLE_CAPABILITIES: [&str; 2] = ["canvas", "sqlite"];
    let mut capabilities = manifest
        .capabilities
        .enabled
        .iter()
        .filter(|capability| !MUTABLE_CAPABILITIES.contains(&capability.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    capabilities.extend(
        enabled_capabilities
            .into_iter()
            .filter(|capability| MUTABLE_CAPABILITIES.contains(&capability.as_str())),
    );
    manifest.capabilities.enabled = capabilities;
    manifest.capabilities.enabled.sort();
    manifest.capabilities.enabled.dedup();
    manifest.defaults.quick_note_directory = quick_note_directory;
    let content = serde_yaml::to_string(&manifest).map_err(|error| error.to_string())?;

    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            "Update workspace settings",
            vec![SemanticCommand::WorkspaceManifestUpdate {
                content,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;
    snapshot_from_workspace(&Workspace::open(Path::new(&root)).map_err(|error| error.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        lattice_core::Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn open_workspace_returns_snapshot_with_resources() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let snapshot = open_workspace(dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(snapshot.title, "Test Workspace");
        assert!(snapshot
            .resources
            .iter()
            .any(|r| r.path.ends_with("Notes.md")));
    }

    #[test]
    fn open_workspace_rejects_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(open_workspace(dir.path().to_string_lossy().into_owned()).is_err());
    }

    #[test]
    fn read_file_returns_contents_within_root() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let content = read_file(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".to_string(),
        )
        .unwrap();
        assert_eq!(content, "# Hi\n");
    }

    #[test]
    fn read_file_rejects_relative_traversal() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        lattice_core::Workspace::init(&ws, "Inner").unwrap();

        let result = read_file(
            ws.to_string_lossy().into_owned(),
            "../secret.txt".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn read_file_rejects_absolute_escape() {
        let dir = init_workspace();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), "nope").unwrap();

        let result = read_file(
            dir.path().to_string_lossy().into_owned(),
            outside.path().to_string_lossy().into_owned(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn read_binary_file_returns_exact_bytes_within_root() {
        let dir = init_workspace();
        let bytes = [0, 159, 146, 150, 255];
        std::fs::write(dir.path().join("image.bin"), bytes).unwrap();

        let content = read_binary_file(
            dir.path().to_string_lossy().into_owned(),
            "image.bin".to_string(),
        )
        .unwrap();
        assert_eq!(content, bytes);
    }

    #[test]
    fn native_resource_commands_expose_inspection_and_text_windows() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Board.canvas"), br#"{"nodes":[]}"#).unwrap();
        std::fs::write(dir.path().join("Note.txt"), "hello native\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let inspection = inspect_resource(root.clone(), "Board.canvas".into()).unwrap();
        assert_eq!(
            inspection.profile,
            lattice_core::ResourceFormatProfile::JsonCanvas
        );
        let window = read_text_window(root, "Note.txt".into(), 0, 5).unwrap();
        assert_eq!(window.content, "hello");
    }

    #[test]
    fn native_resource_command_preserves_structured_read_errors() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("blob.bin"), [0, 159, 146, 150]).unwrap();
        let result = read_text_window(
            dir.path().to_string_lossy().into_owned(),
            "blob.bin".into(),
            0,
            4,
        );
        assert!(matches!(
            result,
            Err(lattice_core::ResourceRuntimeError::BinaryResource { .. })
        ));
    }

    #[test]
    fn decode_header_restores_percent_encoded_unicode_paths() {
        assert_eq!(
            decode_header("%2FUsers%2Fwill%2FLattice%2F%E6%97%A5%E8%A8%98.md").unwrap(),
            "/Users/will/Lattice/日記.md"
        );
    }

    #[test]
    fn read_page_returns_content_and_revision() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let page = read_page(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".to_string(),
        )
        .unwrap();
        assert_eq!(page.content, "# Hi\n");
        assert!(page.revision.starts_with("sha256:"));
    }

    #[test]
    fn apply_page_update_writes_content_and_returns_new_revision() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let before = read_page(root.clone(), "Notes.md".to_string()).unwrap();
        let after_revision = apply_page_update(
            root.clone(),
            "Notes.md".to_string(),
            "# Hi, edited\n".to_string(),
            before.revision,
        )
        .unwrap();

        let after = read_page(root, "Notes.md".to_string()).unwrap();
        assert_eq!(after.content, "# Hi, edited\n");
        assert_eq!(after.revision, after_revision);
    }

    #[test]
    fn list_resources_matches_open_workspace_scan() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let resources = list_resources(root).unwrap();
        assert!(resources.iter().any(|r| r.path.ends_with("Notes.md")));
    }

    #[test]
    fn create_page_writes_new_file_and_rejects_existing() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        let revision = create_page(
            root.clone(),
            "Notes (conflict 2026-07-15).md".to_string(),
            "# Local copy\n".to_string(),
            None,
            None,
        )
        .unwrap();
        assert!(revision.starts_with("sha256:"));

        let content =
            read_file(root.clone(), "Notes (conflict 2026-07-15).md".to_string()).unwrap();
        assert_eq!(content, "# Local copy\n");

        let err = create_page(
            root,
            "Notes (conflict 2026-07-15).md".to_string(),
            "# Again\n".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn create_page_from_template_substitutes_placeholders() {
        let dir = init_workspace();
        std::fs::create_dir_all(dir.path().join("Templates")).unwrap();
        std::fs::write(
            dir.path().join("Templates/Daily.md"),
            "# {{title}}\n\n{{date}}\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        create_page(
            root.clone(),
            "Notes/Sync.md".to_string(),
            String::new(),
            Some("Templates/Daily.md".to_string()),
            Some("Sync".to_string()),
        )
        .unwrap();

        let content = read_file(root, "Notes/Sync.md".to_string()).unwrap();
        assert!(content.starts_with("# Sync\n\n"));
        assert!(content.contains('-'));
    }

    #[test]
    fn create_asset_is_page_relative_collision_safe_and_undoable() {
        let dir = init_workspace();
        std::fs::create_dir_all(dir.path().join("Notes")).unwrap();
        std::fs::write(dir.path().join("Notes/Idea.md"), "# Idea\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let first = create_asset_inner(
            root.clone(),
            "Notes/Idea.md".to_string(),
            "diagram.png".to_string(),
            vec![1, 2, 3],
        )
        .unwrap();
        let second = create_asset_inner(
            root.clone(),
            "Notes/Idea.md".to_string(),
            "diagram.png".to_string(),
            vec![4, 5, 6],
        )
        .unwrap();

        assert_eq!(first, "assets/diagram.png");
        assert_eq!(second, "assets/diagram 2.png");
        assert_eq!(
            std::fs::read(dir.path().join("Notes/assets/diagram.png")).unwrap(),
            vec![1, 2, 3]
        );

        undo_last(root).unwrap();
        assert!(!dir.path().join("Notes/assets/diagram 2.png").exists());
    }

    #[test]
    fn undo_last_reverts_the_most_recent_transaction() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        create_page(
            root.clone(),
            "Inbox/Note.md".to_string(),
            "# Note\n".to_string(),
            None,
            None,
        )
        .unwrap();
        assert!(dir.path().join("Inbox/Note.md").exists());

        let summary = undo_last(root).unwrap();
        assert_eq!(summary, Some("Create page Inbox/Note.md".to_string()));
        assert!(!dir.path().join("Inbox/Note.md").exists());
    }

    #[test]
    fn undo_last_returns_none_when_history_is_empty() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        assert_eq!(undo_last(root).unwrap(), None);
    }

    #[test]
    fn rename_resource_uses_command_history() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Before.md"), "# Before\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        rename_resource(
            root.clone(),
            "Before.md".to_string(),
            "After.md".to_string(),
        )
        .unwrap();

        assert!(!dir.path().join("Before.md").exists());
        assert!(dir.path().join("After.md").exists());
        let history = list_history(root, 10).unwrap();
        assert_eq!(history[0].summary, "Rename Before.md to After.md");
    }

    #[test]
    fn apply_page_update_reports_stale_revision() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let result = apply_page_update(
            root,
            "Notes.md".to_string(),
            "# Hi, edited\n".to_string(),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        );

        let err = result.unwrap_err();
        assert!(
            err.starts_with(STALE_REVISION_PREFIX),
            "expected a STALE_REVISION-prefixed error, got: {err}"
        );
    }
}
