//! Embedded multimodal derived-data store contract for Lattice.
//!
//! This crate defines the LanceDB-facing trait boundary and Lattice-owned row
//! types for workspace search vectors (ADR 0060/0061). Lance I/O is implemented
//! in `EmbeddedLanceStore` (T2); T1 ships types, path helpers, and the trait.

mod error;
mod paths;
mod store;
mod types;

pub use error::{LanceError, Result};
pub use paths::{search_elements_dataset_path, SEARCH_ELEMENTS_DATASET};
pub use store::{MultimodalStore, UnsupportedStore};
pub use types::{
    Commit, DatasetRef, DatasetSnapshot, SearchElementBatch, SearchElementRow, SearchHit,
    SearchRequest, SearchResults, DEFAULT_ELEMENT_KIND, SEARCH_ELEMENTS_DATASET_ID,
};
