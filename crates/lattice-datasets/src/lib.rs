//! Lattice `.dataset` analytical package create/open/validate and Parquet facts.
//!
//! See `docs/11-analytical-data-arrow-duckdb-parquet.md` for the on-disk layout.
//!
//! # Third-party licenses
//!
//! Parquet I/O uses the Apache Arrow Rust crates (`arrow`, `parquet`), licensed
//! under **Apache-2.0**. CSV parsing for import uses `arrow`'s CSV feature
//! (also Apache-2.0) and the `csv` crate (Unlicense OR MIT) transitively.

mod error;
mod import;
mod manifest;
mod package;
mod partition;

pub use error::Error;
pub use manifest::{
    dataset_manifest_path, DatasetManifest, PartitionEntry, DATASET_FORMAT,
    DATASET_MANIFEST_FILENAME, SUPPORTED_VERSION,
};
pub use package::{
    validate_package_layout, Dataset, ANNOTATIONS_FILENAME, FACTS_DIR, QUERIES_DIR, README_FILENAME,
    VIEWS_DIR,
};
pub use partition::{
    hive_facts_relative_path, hive_keys_from_relative_path, normalize_facts_relative,
    parse_partition_key_specs, DEFAULT_PARTITION_FILE,
};

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
