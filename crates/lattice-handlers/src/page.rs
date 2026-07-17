use std::path::{Path, PathBuf};

use lattice_commands::{Command as SemanticCommand, CommandEngine, Transaction};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde::Serialize;

use crate::error::command_error_to_string;
use crate::path::resolve_within_root;

/// A page's content plus the content-hash revision it was read at, so the
/// editor can round-trip that revision back as `apply_page_update`'s
/// `base_revision` (optimistic concurrency, ADR 0007).
#[derive(Debug, Serialize)]
pub struct PageContent {
    pub content: String,
    pub revision: String,
}

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

/// Create a new page at `rel_path`.
///
/// When `template_path` is set, the template body is read from the workspace,
/// placeholders are substituted, and the result is written through the semantic
/// command core. `content` is used only for blank creates (`template_path` absent).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::STALE_REVISION_PREFIX;
    use lattice_core::Workspace;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
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

        let content = std::fs::read_to_string(dir.path().join("Notes (conflict 2026-07-15).md"))
            .unwrap();
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

        let content = std::fs::read_to_string(dir.path().join("Notes/Sync.md")).unwrap();
        assert!(content.starts_with("# Sync\n\n"));
        assert!(content.contains('-'));
    }
}
