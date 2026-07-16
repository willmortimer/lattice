use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::manifest::{manifest_path, WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME};
use crate::{Error, Resource, ResourceKind, Result};

/// Directory name of the operational (derived, rebuildable) state layer.
pub const OPERATIONAL_DIR: &str = ".lattice";

/// An opened Lattice workspace: a root directory plus its parsed manifest.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    manifest: WorkspaceManifest,
}

impl Workspace {
    /// Create a new workspace at `root`, which may or may not exist but
    /// must not already contain a manifest.
    pub fn init(root: &Path, title: impl Into<String>) -> Result<Workspace> {
        Self::init_with_manifest(root, WorkspaceManifest::new(title))
    }

    pub(crate) fn init_with_manifest(
        root: &Path,
        manifest: WorkspaceManifest,
    ) -> Result<Workspace> {
        let manifest_file = manifest_path(root);
        if manifest_file.exists() {
            return Err(Error::WorkspaceExists {
                path: root.to_path_buf(),
            });
        }
        std::fs::create_dir_all(root).map_err(|e| Error::io(root, e))?;
        manifest.save(&manifest_file)?;
        Ok(Workspace {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// Open the workspace whose root is exactly `root`.
    pub fn open(root: &Path) -> Result<Workspace> {
        let manifest_file = manifest_path(root);
        if !manifest_file.exists() {
            return Err(Error::WorkspaceNotFound(root.to_path_buf()));
        }
        let manifest = WorkspaceManifest::load(&manifest_file)?;
        Ok(Workspace {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// Walk up from `start` to find the nearest enclosing workspace.
    pub fn discover(start: &Path) -> Result<Workspace> {
        for dir in start.ancestors() {
            if manifest_path(dir).exists() {
                return Workspace::open(dir);
            }
        }
        Err(Error::WorkspaceNotFound(start.to_path_buf()))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &WorkspaceManifest {
        &self.manifest
    }

    /// Enumerate resources in the canonical layer.
    ///
    /// Package directories (`.data/`, `.ink/`, …) are yielded as a single
    /// resource and not descended into. `.lattice/`, VCS metadata, and
    /// other hidden entries are skipped. The workspace manifest itself is
    /// not a resource.
    pub fn scan(&self) -> Result<Vec<Resource>> {
        let mut resources = Vec::new();
        let mut walker = WalkDir::new(&self.root)
            .min_depth(1)
            .sort_by_file_name()
            .into_iter();

        while let Some(entry) = walker.next() {
            let entry = entry.map_err(|e| {
                let path = e
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| self.root.clone());
                Error::io(path.clone(), e.into())
            })?;
            let name = entry.file_name().to_str().unwrap_or("");
            if name.starts_with('.') {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
            let is_dir = entry.file_type().is_dir();
            let kind = ResourceKind::classify(entry.path(), is_dir);
            let rel = entry
                .path()
                .strip_prefix(&self.root)
                .expect("walked paths are under root")
                .to_path_buf();
            if rel.as_os_str() == WORKSPACE_MANIFEST_FILENAME {
                continue;
            }
            if is_dir {
                if kind.is_package() {
                    walker.skip_current_dir();
                    resources.push(Resource { path: rel, kind });
                } else {
                    // Ordinary folder: list it (so empty template dirs appear
                    // in the sidebar) and keep descending for children.
                    resources.push(Resource {
                        path: rel,
                        kind: ResourceKind::Folder,
                    });
                }
                continue;
            }
            resources.push(Resource { path: rel, kind });
        }
        Ok(resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::init(dir.path(), "Test Workspace").unwrap();
        (dir, ws)
    }

    #[test]
    fn init_then_open_roundtrips_manifest() {
        let (dir, ws) = temp_workspace();
        let reopened = Workspace::open(dir.path()).unwrap();
        assert_eq!(reopened.manifest(), ws.manifest());
        assert_eq!(reopened.manifest().title, "Test Workspace");
    }

    #[test]
    fn init_refuses_existing_workspace() {
        let (dir, _ws) = temp_workspace();
        assert!(matches!(
            Workspace::init(dir.path(), "Again"),
            Err(Error::WorkspaceExists { .. })
        ));
    }

    #[test]
    fn discover_walks_up_from_nested_path() {
        let (dir, _ws) = temp_workspace();
        let nested = dir.path().join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        let ws = Workspace::discover(&nested).unwrap();
        assert_eq!(ws.root(), dir.path());
    }

    #[test]
    fn scan_classifies_resources_and_skips_hidden() {
        let (dir, ws) = temp_workspace();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Notes")).unwrap();
        std::fs::write(root.join("Notes/Ideas.md"), "# Ideas\n").unwrap();
        std::fs::write(root.join("Board.canvas"), "{}").unwrap();
        std::fs::write(root.join("Analysis.ipynb"), "{}").unwrap();
        std::fs::write(root.join("Refresh.workflow.yaml"), "").unwrap();
        std::fs::write(root.join("photo.png"), []).unwrap();
        std::fs::create_dir_all(root.join("CRM.data")).unwrap();
        std::fs::write(root.join("CRM.data/app.yaml"), "").unwrap();
        std::fs::create_dir_all(root.join(".lattice/cache")).unwrap();
        std::fs::write(root.join(".lattice/index.sqlite"), []).unwrap();

        let resources = ws.scan().unwrap();
        let kind_of = |p: &str| {
            resources
                .iter()
                .find(|r| r.path == Path::new(p))
                .map(|r| r.kind)
        };
        assert_eq!(kind_of("Notes/Ideas.md"), Some(ResourceKind::Page));
        assert_eq!(kind_of("Board.canvas"), Some(ResourceKind::Canvas));
        assert_eq!(kind_of("Analysis.ipynb"), Some(ResourceKind::Notebook));
        assert_eq!(
            kind_of("Refresh.workflow.yaml"),
            Some(ResourceKind::Workflow)
        );
        assert_eq!(kind_of("photo.png"), Some(ResourceKind::File));
        assert_eq!(kind_of("CRM.data"), Some(ResourceKind::DataApp));
        // package contents and hidden dirs are not resources
        assert_eq!(kind_of("CRM.data/app.yaml"), None);
        assert!(resources
            .iter()
            .all(|r| !r.path.starts_with(OPERATIONAL_DIR)));
        // the manifest itself is not a resource
        assert_eq!(kind_of(WORKSPACE_MANIFEST_FILENAME), None);
    }
}
