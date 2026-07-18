use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lattice_commands::{
    Command as SemanticCommand, CommandEngine, Transaction,
};
use lattice_core::{
    inspect_resource as inspect_native_resource,
    read_resource_range as read_native_resource_range, read_text_window as read_native_text_window,
    ResourceKind, ResourceRuntimeError, Workspace,
};
use lattice_handlers::{self, join_within_root, snapshot_from_workspace};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request, Response};

pub use lattice_handlers::{
    command_error_to_string, resolve_within_root, LatticeHomeInfo, PageContent,
    WorkspaceProvisionResult, WorkspaceSnapshot,
};
#[allow(unused_imports)]
pub use lattice_handlers::STALE_REVISION_PREFIX;

const MAX_EDITOR_ASSET_BYTES: usize = 8 * 1024 * 1024;

#[tauri::command]
pub fn open_workspace(path: String) -> Result<WorkspaceSnapshot, String> {
    lattice_handlers::open_workspace(path)
}

#[tauri::command]
pub fn list_resources(root: String) -> Result<Vec<lattice_core::Resource>, String> {
    lattice_handlers::list_resources(root)
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

/// Read a page and the revision it was read at, in one round trip.
#[tauri::command]
pub fn read_page(root: String, rel_path: String) -> Result<PageContent, String> {
    lattice_handlers::read_page(root, rel_path)
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
    lattice_handlers::apply_page_update(root, rel_path, content, base_revision)
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
    lattice_handlers::create_page(root, rel_path, content, template_path, title)
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

/// Delete a resource through the semantic command core (files go to Trash).
#[tauri::command]
pub fn delete_resource(root: String, path: String) -> Result<(), String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            format!("Delete {path}"),
            vec![SemanticCommand::ResourceDelete {
                path: PathBuf::from(path),
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(())
}

/// Move a resource into an existing directory through the semantic command core.
#[tauri::command]
pub fn move_resource(root: String, from: String, to_dir: String) -> Result<(), String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            format!("Move {from} into {to_dir}"),
            vec![SemanticCommand::ResourceMove {
                from: PathBuf::from(from),
                to_dir: PathBuf::from(to_dir),
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(())
}

fn duplicate_destination_path(
    store: &NativeWorkspaceStore,
    source: &Path,
) -> Result<PathBuf, String> {
    let parent = source.parent().unwrap_or_else(|| Path::new(""));
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "cannot duplicate a resource without a name".to_string())?;
    let extension = source.extension().and_then(|value| value.to_str());

    let mut candidate = parent.join(match extension {
        Some(extension) => format!("{stem} copy.{extension}"),
        None => format!("{stem} copy"),
    });
    let mut suffix = 2usize;
    while store.metadata(&candidate).is_ok() {
        let next_name = match extension {
            Some(extension) => format!("{stem} copy {suffix}.{extension}"),
            None => format!("{stem} copy {suffix}"),
        };
        candidate = parent.join(next_name);
        suffix += 1;
    }
    Ok(candidate)
}

/// Duplicate a file resource at a unique sibling path (`Foo copy.md`, `Foo copy 2.md`, …).
#[tauri::command]
pub fn duplicate_resource(root: String, path: String) -> Result<String, String> {
    let (canonical_root, _) = resolve_within_root(&root, &path)?;
    let source = PathBuf::from(&path);
    let store = NativeWorkspaceStore::new(&canonical_root);
    let metadata = store
        .metadata(&source)
        .map_err(|err| err.to_string())?;
    if metadata.is_dir {
        return Err(format!("cannot duplicate directory {path:?}"));
    }

    let bytes = store.read(&source).map_err(|err| err.to_string())?;
    let destination = duplicate_destination_path(&store, &source)?;
    let kind = ResourceKind::classify(&source, false);
    let command = if kind == ResourceKind::Page {
        SemanticCommand::PageCreate {
            path: destination.clone(),
            content: String::from_utf8(bytes).map_err(|_| {
                format!("page {path:?} is not valid UTF-8 and cannot be duplicated as a page")
            })?,
        }
    } else {
        SemanticCommand::ResourceCreate {
            path: destination.clone(),
            content: bytes,
        }
    };

    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            format!("Duplicate {} to {}", source.display(), destination.display()),
            vec![command],
        ))
        .map_err(command_error_to_string)?;

    Ok(destination
        .to_string_lossy()
        .replace('\\', "/"))
}

/// Create an empty folder beneath the workspace root.
///
/// Folders are ordinary directories discovered by workspace scan. There is
/// no `FolderCreate` semantic command yet, so this mirrors template
/// provisioning (`create_dir`) rather than inventing an undeclared command.
/// The operation is not recorded in command history and is not undoable.
#[tauri::command]
pub fn create_folder(root: String, path: String) -> Result<(), String> {
    let (canonical_root, relative) = join_within_root(&root, &path)?;
    let target = canonical_root.join(&relative);
    if target.exists() {
        return Err(format!("{path:?} already exists"));
    }
    let parent = target
        .parent()
        .filter(|parent| parent.starts_with(&canonical_root))
        .ok_or_else(|| format!("invalid folder path {path:?}"))?;
    if parent == canonical_root {
        // Creating a top-level folder is allowed.
    } else if !parent.is_dir() {
        return Err(format!(
            "parent directory {} does not exist",
            parent.strip_prefix(&canonical_root)
                .unwrap_or(parent)
                .to_string_lossy()
        ));
    }
    std::fs::create_dir(&target).map_err(|err| err.to_string())
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

/// Explicitly initialize Lattice home, provisioning a workspace only when no valid
/// workspace exists. Uses the First Look demo template when `LATTICE_DEV_HOME` is
/// set; otherwise provisions Personal. Ordinary startup never calls this command.
#[tauri::command]
pub fn ensure_home() -> Result<LatticeHomeInfo, String> {
    lattice_handlers::ensure_home()
}

#[tauri::command]
pub fn create_workspace(
    path: String,
    title: Option<String>,
    template: String,
    set_default: bool,
    initialize_existing: bool,
) -> Result<WorkspaceProvisionResult, String> {
    lattice_handlers::create_workspace(path, title, template, set_default, initialize_existing)
}

/// Built-in workspace templates for the New Workspace gallery and First Look sample.
#[tauri::command]
pub fn list_templates() -> Vec<lattice_core::TemplateDescriptor> {
    lattice_handlers::list_templates()
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
            decode_header("%2FUsers%2Fexample%2FLattice%2F%E6%97%A5%E8%A8%98.md").unwrap(),
            "/Users/example/Lattice/日記.md"
        );
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
    fn delete_resource_uses_command_history() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Doomed.md"), "# Doomed\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        delete_resource(root.clone(), "Doomed.md".to_string()).unwrap();

        assert!(!dir.path().join("Doomed.md").exists());
        let history = list_history(root, 10).unwrap();
        assert_eq!(history[0].summary, "Delete Doomed.md");
    }

    #[test]
    fn move_resource_uses_command_history() {
        let dir = init_workspace();
        std::fs::create_dir(dir.path().join("Inbox")).unwrap();
        std::fs::write(dir.path().join("Note.md"), "# Note\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        move_resource(
            root.clone(),
            "Note.md".to_string(),
            "Inbox".to_string(),
        )
        .unwrap();

        assert!(!dir.path().join("Note.md").exists());
        assert!(dir.path().join("Inbox/Note.md").exists());
        let history = list_history(root, 10).unwrap();
        assert_eq!(history[0].summary, "Move Note.md into Inbox");
    }

    #[test]
    fn duplicate_resource_creates_collision_safe_copy() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Note.md"), "# Note\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let first = duplicate_resource(root.clone(), "Note.md".to_string()).unwrap();
        assert_eq!(first, "Note copy.md");
        assert_eq!(
            read_file(root.clone(), "Note copy.md".to_string()).unwrap(),
            "# Note\n"
        );

        let second = duplicate_resource(root, "Note.md".to_string()).unwrap();
        assert_eq!(second, "Note copy 2.md");
    }

    #[test]
    fn create_folder_adds_empty_directory() {
        let dir = init_workspace();
        std::fs::create_dir(dir.path().join("Projects")).unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        create_folder(root.clone(), "Projects/New".to_string()).unwrap();

        assert!(dir.path().join("Projects/New").is_dir());
        let resources = list_resources(root).unwrap();
        assert!(resources
            .iter()
            .any(|resource| resource.path == PathBuf::from("Projects/New")));
    }
}
