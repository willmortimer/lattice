use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::Error;
use crate::manifest::{dataset_manifest_path, DatasetManifest, DATASET_MANIFEST_FILENAME};
use crate::Result;

pub const README_FILENAME: &str = "README.md";
pub const FACTS_DIR: &str = "facts";
pub const VIEWS_DIR: &str = "views";
pub const QUERIES_DIR: &str = "queries";
pub const ANNOTATIONS_FILENAME: &str = "annotations.sqlite";

/// An opened or newly created `.dataset` analytical package.
#[derive(Debug, Clone)]
pub struct Dataset {
    path: PathBuf,
    manifest: DatasetManifest,
}

impl Dataset {
    /// Create a new empty `.dataset` package with the canonical layout.
    pub fn create(
        package_path: &Path,
        title: &str,
        description: Option<&str>,
    ) -> Result<Self> {
        if package_path.exists() {
            return Err(Error::invalid_package(
                package_path,
                "package path already exists",
            ));
        }

        std::fs::create_dir_all(package_path).map_err(|source| Error::io(package_path, source))?;

        for folder in [FACTS_DIR, VIEWS_DIR, QUERIES_DIR] {
            let dir = package_path.join(folder);
            std::fs::create_dir_all(&dir).map_err(|source| Error::io(&dir, source))?;
        }

        let readme_path = package_path.join(README_FILENAME);
        let readme = format!("# {title}\n\nAnalytical dataset package. Facts land in `{FACTS_DIR}/`.\n");
        std::fs::write(&readme_path, readme).map_err(|source| Error::io(&readme_path, source))?;

        let manifest = DatasetManifest::new(
            title,
            description.map(str::to_string),
        );
        let manifest_path = dataset_manifest_path(package_path);
        manifest.save(&manifest_path)?;

        Ok(Dataset {
            path: package_path.to_path_buf(),
            manifest,
        })
    }

    /// Open an existing `.dataset` package after validating required layout.
    pub fn open(package_path: &Path) -> Result<Self> {
        validate_package_layout(package_path)?;

        let manifest_path = dataset_manifest_path(package_path);
        let manifest = DatasetManifest::load(&manifest_path)?;

        Ok(Dataset {
            path: package_path.to_path_buf(),
            manifest,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn title(&self) -> &str {
        &self.manifest.title
    }

    pub fn description(&self) -> Option<&str> {
        self.manifest.description.as_deref()
    }

    pub fn manifest(&self) -> &DatasetManifest {
        &self.manifest
    }

    pub(crate) fn manifest_mut(&mut self) -> &mut DatasetManifest {
        &mut self.manifest
    }

    /// Content hash of `dataset.yaml` for optimistic concurrency later.
    pub fn package_revision(&self) -> Result<String> {
        let manifest_path = dataset_manifest_path(&self.path);
        let bytes = std::fs::read(&manifest_path).map_err(|source| Error::io(&manifest_path, source))?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{}", hex::encode(digest)))
    }
}

/// Validate the on-disk layout of a `.dataset` package without opening it.
pub fn validate_package_layout(package_path: &Path) -> Result<()> {
    if !package_path.is_dir() {
        return Err(Error::invalid_package(
            package_path,
            "expected a package directory",
        ));
    }

    for (label, path) in [
        (DATASET_MANIFEST_FILENAME, dataset_manifest_path(package_path)),
        (README_FILENAME, package_path.join(README_FILENAME)),
        (FACTS_DIR, package_path.join(FACTS_DIR)),
    ] {
        let missing = if label == FACTS_DIR {
            !path.is_dir()
        } else {
            !path.is_file()
        };
        if missing {
            return Err(Error::invalid_package(
                package_path,
                format!("missing required {label}"),
            ));
        }
    }

    for optional in [VIEWS_DIR, QUERIES_DIR] {
        let path = package_path.join(optional);
        if path.exists() && !path.is_dir() {
            return Err(Error::invalid_package(
                package_path,
                format!("{optional} must be a directory when present"),
            ));
        }
    }

    let annotations = package_path.join(ANNOTATIONS_FILENAME);
    if annotations.exists() && !annotations.is_file() {
        return Err(Error::invalid_package(
            package_path,
            format!("{ANNOTATIONS_FILENAME} must be a file when present"),
        ));
    }

    Ok(())
}
