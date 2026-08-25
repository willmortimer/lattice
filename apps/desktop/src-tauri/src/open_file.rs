//! Classify Finder / document-open paths for the desktop shell.
//!
//! Empty `path` on [`OpenResourcePayload`] means “open this workspace root”
//! rather than select a file. Paths with no `.lattice` / `lattice.yaml`
//! ancestor emit [`OpenUnregisteredPayload`] so the UI is not a silent no-op.

use std::path::{Path, PathBuf};

use crate::deep_link::{OpenResourcePayload, OpenUnregisteredPayload, UnregisteredKind};

const LATTICE_DIR: &str = ".lattice";
const LATTICE_MANIFEST: &str = "lattice.yaml";

/// Outcome of resolving a local file or folder the OS asked Lattice to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenFileAction {
    Resource(OpenResourcePayload),
    Unregistered(OpenUnregisteredPayload),
}

/// Map an OS-opened path to a workspace resource or an unregistered payload.
pub fn open_payload_for_file(path: &Path) -> OpenFileAction {
    let file = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    match find_workspace_root(&file) {
        Some(root) => {
            let relative = file
                .strip_prefix(&root)
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            OpenFileAction::Resource(OpenResourcePayload {
                root: root.display().to_string(),
                path: relative,
            })
        }
        None => OpenFileAction::Unregistered(OpenUnregisteredPayload {
            path: file.display().to_string(),
            kind: unregistered_kind(&file),
        }),
    }
}

fn find_workspace_root(file: &Path) -> Option<PathBuf> {
    file.ancestors()
        .find(|ancestor| is_workspace_root(ancestor))
        .map(Path::to_path_buf)
}

fn is_workspace_root(dir: &Path) -> bool {
    dir.join(LATTICE_DIR).is_dir() || dir.join(LATTICE_MANIFEST).is_file()
}

fn unregistered_kind(path: &Path) -> UnregisteredKind {
    if path.is_dir() {
        UnregisteredKind::Folder
    } else {
        UnregisteredKind::File
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn display_canonical(path: &Path) -> String {
        path.canonicalize().unwrap().display().to_string()
    }

    #[test]
    fn file_inside_workspace_emits_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(LATTICE_DIR)).unwrap();
        let notes = dir.path().join("Notes.md");
        fs::write(&notes, "hello").unwrap();

        match open_payload_for_file(&notes) {
            OpenFileAction::Resource(payload) => {
                assert_eq!(payload.root, display_canonical(dir.path()));
                assert_eq!(payload.path, "Notes.md");
            }
            other => panic!("expected resource, got {other:?}"),
        }
    }

    #[test]
    fn nested_file_inside_workspace_keeps_relative_segments() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(LATTICE_DIR)).unwrap();
        fs::create_dir(dir.path().join("pages")).unwrap();
        let notes = dir.path().join("pages").join("Hello.md");
        fs::write(&notes, "hello").unwrap();

        match open_payload_for_file(&notes) {
            OpenFileAction::Resource(payload) => {
                assert_eq!(payload.path, "pages/Hello.md");
            }
            other => panic!("expected resource, got {other:?}"),
        }
    }

    #[test]
    fn workspace_folder_emits_empty_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(LATTICE_DIR)).unwrap();

        match open_payload_for_file(dir.path()) {
            OpenFileAction::Resource(payload) => {
                assert_eq!(payload.root, display_canonical(dir.path()));
                assert_eq!(payload.path, "");
            }
            other => panic!("expected workspace root, got {other:?}"),
        }
    }

    #[test]
    fn lattice_yaml_marks_workspace_when_dot_lattice_missing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(LATTICE_MANIFEST), "name: yaml-only\n").unwrap();

        match open_payload_for_file(dir.path()) {
            OpenFileAction::Resource(payload) => {
                assert_eq!(payload.root, display_canonical(dir.path()));
                assert_eq!(payload.path, "");
            }
            other => panic!("expected workspace root, got {other:?}"),
        }
    }

    #[test]
    fn path_with_no_lattice_is_unregistered() {
        let dir = tempfile::tempdir().unwrap();
        let stray = dir.path().join("stray.md");
        fs::write(&stray, "no workspace").unwrap();

        match open_payload_for_file(&stray) {
            OpenFileAction::Unregistered(payload) => {
                assert_eq!(payload.path, display_canonical(&stray));
                assert_eq!(payload.kind, UnregisteredKind::File);
            }
            other => panic!("expected unregistered, got {other:?}"),
        }
    }

    #[test]
    fn folder_with_no_lattice_is_unregistered() {
        let dir = tempfile::tempdir().unwrap();

        match open_payload_for_file(dir.path()) {
            OpenFileAction::Unregistered(payload) => {
                assert_eq!(payload.path, display_canonical(dir.path()));
                assert_eq!(payload.kind, UnregisteredKind::Folder);
            }
            other => panic!("expected unregistered, got {other:?}"),
        }
    }

    #[test]
    fn unregistered_file_does_not_wrap_parent_folder() {
        let dir = tempfile::tempdir().unwrap();
        let stray = dir.path().join("notes.md");
        fs::write(&stray, "hello").unwrap();

        match open_payload_for_file(&stray) {
            OpenFileAction::Unregistered(payload) => {
                assert_eq!(payload.kind, UnregisteredKind::File);
                assert_ne!(payload.path, display_canonical(dir.path()));
            }
            other => panic!("expected unregistered file, got {other:?}"),
        }
    }
}
