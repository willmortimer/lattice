use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The primary product resource types (docs/01-product-vision.md).
///
/// Classification is by naming convention: package directories use a
/// dotted suffix (`.data/`, `.ink/`, …) and carry their own manifest;
/// everything unrecognized is an ordinary [`ResourceKind::File`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    /// Narrative Markdown content (`.md`).
    Page,
    /// JSON Canvas spatial composition (`.canvas`).
    Canvas,
    /// Mutable typed relational data package (`.data/`).
    DataApp,
    /// Analytical dataset package (`.dataset/`).
    Dataset,
    /// Jupyter-compatible notebook (`.ipynb`).
    Notebook,
    /// Open stroke-data ink package (`.ink/`).
    Ink,
    /// Sandboxed HTML/CSS/JS mini-application package (`.artifact/`).
    Artifact,
    /// Full source-backed application package (`.app/`).
    App,
    /// Inspectable automation resource (`.workflow.yaml`).
    Workflow,
    /// Executable task package (`.task/`).
    Task,
    /// Generated output with declared inputs and builder (`.derived.yaml`).
    Derived,
    /// An ordinary directory (scaffolding / navigation). Empty folders from
    /// templates appear as this so the sidebar tree can show them before
    /// they contain pages.
    Folder,
    /// Any ordinary content without a special native model.
    File,
}

impl ResourceKind {
    /// Package kinds are directories treated as a single resource.
    pub fn is_package(self) -> bool {
        matches!(
            self,
            ResourceKind::DataApp
                | ResourceKind::Dataset
                | ResourceKind::Ink
                | ResourceKind::Artifact
                | ResourceKind::App
                | ResourceKind::Task
        )
    }

    /// The manifest filename expected inside a package directory.
    pub fn package_manifest(self) -> Option<&'static str> {
        match self {
            ResourceKind::DataApp => Some("app.yaml"),
            ResourceKind::Dataset => Some("dataset.yaml"),
            ResourceKind::Ink => Some("manifest.json"),
            ResourceKind::Artifact => Some("artifact.yaml"),
            ResourceKind::App => Some("lattice-app.yaml"),
            ResourceKind::Task => Some("task.yaml"),
            _ => None,
        }
    }

    /// Classify a path. `is_dir` distinguishes package directories from
    /// files that merely share an extension.
    pub fn classify(path: &Path, is_dir: bool) -> ResourceKind {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if is_dir {
            return match name.rsplit_once('.').map(|(_, ext)| ext) {
                Some("data") => ResourceKind::DataApp,
                Some("dataset") => ResourceKind::Dataset,
                Some("ink") => ResourceKind::Ink,
                Some("artifact") => ResourceKind::Artifact,
                Some("app") => ResourceKind::App,
                Some("task") => ResourceKind::Task,
                _ => ResourceKind::File,
            };
        }
        if name.ends_with(".workflow.yaml") || name.ends_with(".workflow.yml") {
            return ResourceKind::Workflow;
        }
        if name.ends_with(".derived.yaml") || name.ends_with(".derived.yml") {
            return ResourceKind::Derived;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") => ResourceKind::Page,
            Some("canvas") => ResourceKind::Canvas,
            Some("ipynb") => ResourceKind::Notebook,
            _ => ResourceKind::File,
        }
    }
}

/// A resource discovered in a workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    /// Path relative to the workspace root.
    pub path: PathBuf,
    pub kind: ResourceKind,
}
