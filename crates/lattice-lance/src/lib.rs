//! Embedded multimodal derived-data store contract for Lattice.
//!
//! This crate defines the LanceDB-facing trait boundary and Lattice-owned row
//! types for workspace search vectors (ADR 0060/0061). Lance I/O is provided by
//! [`EmbeddedLanceStore`].

mod arrow_convert;
mod embedded;
mod error;
mod paths;
mod store;
mod sync;
mod types;

pub use embedded::EmbeddedLanceStore;
pub use error::{LanceError, Result};
pub use paths::{
    search_elements_dataset_path, search_elements_index_dir, SEARCH_ELEMENTS_DATASET,
    SEARCH_ELEMENTS_TABLE,
};
pub use store::{MultimodalStore, UnsupportedStore};
pub use sync::block_on;
pub use types::{
    Commit, DatasetRef, DatasetSnapshot, SearchElementBatch, SearchElementRow, SearchHit,
    SearchRequest, SearchResults, DEFAULT_ELEMENT_KIND, SEARCH_ELEMENTS_DATASET_ID,
};
