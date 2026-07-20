//! Lattice `.dataset` analytical package create/open/validate.
//!
//! See `docs/11-analytical-data-arrow-duckdb-parquet.md` for the on-disk layout.

mod error;
mod manifest;
mod package;

pub use error::Error;
pub use manifest::{
    dataset_manifest_path, DatasetManifest, DATASET_FORMAT, DATASET_MANIFEST_FILENAME,
    SUPPORTED_VERSION,
};
pub use package::{
    validate_package_layout, Dataset, ANNOTATIONS_FILENAME, FACTS_DIR, QUERIES_DIR, README_FILENAME,
    VIEWS_DIR,
};

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
