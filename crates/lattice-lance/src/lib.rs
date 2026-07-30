//! Embedded multimodal derived-data store contract for Lattice.
//!
//! This crate defines the LanceDB-facing trait boundary and Lattice-owned row
//! types for workspace search vectors (ADR 0060/0061). Lance I/O is provided by
//! [`EmbeddedLanceStore`].

mod agent_memory;
mod arrow_convert;
mod embedded;
mod error;
mod paths;
mod store;
mod sync;
mod types;

pub use agent_memory::{
    AgentMemoryHit, AgentMemoryRecallRequest, AgentMemoryRecallResults, AgentMemoryRow,
    AgentMemoryStore, AGENT_MEMORY_DATASET_ID, AGENT_MEMORY_EMBEDDING_WIDTH,
};
pub use embedded::EmbeddedLanceStore;
pub use error::{LanceError, Result};
pub use paths::{
    agent_memory_dataset_path, search_elements_dataset_path, search_elements_index_dir,
    AGENT_MEMORY_DATASET, AGENT_MEMORY_TABLE, SEARCH_ELEMENTS_DATASET, SEARCH_ELEMENTS_TABLE,
};
pub use store::{MultimodalStore, UnsupportedStore};
pub use sync::block_on;
pub use types::{
    Commit, DatasetRef, DatasetSnapshot, DerivedDatasetSnapshot, SearchElementBatch,
    SearchElementRow, SearchHit, SearchRequest, SearchResults, DEFAULT_ELEMENT_KIND,
    SEARCH_ELEMENTS_DATASET_ID, SEARCH_ELEMENTS_PIPELINE_VERSION,
};
