//! Derived workspace search index for Lattice.
//!
//! The index lives at `<workspace>/.lattice/index.sqlite` and is fully
//! rebuildable from generic resources on disk. Text extraction is deliberately
//! bounded; binary resources retain searchable names and runtime metadata.

mod catalog;
mod chunks;
mod embedding;
mod error;
mod extract;
mod hybrid;
mod index;
mod lexical;
mod links;
mod paths;
mod provenance;
mod record;
mod schema;
mod semantic;
mod types;
mod vector;

pub use chunks::{chunk_resource, SearchChunkDraft, CHUNKER_VERSION};
pub use embedding::{default_chunker_version, ChunkEmbeddingState, EmbeddingNamespace};
pub use error::{Error, Result};
pub use extract::{
    extract_structured_paths, parse_page, ExtractedLink, Heading, LinkKind, PageIndexData,
    StructuredExtraction, StructuredFormat, StructuredPath,
};
pub use hybrid::{
    diversify_by_resource, reciprocal_rank_fuse, FusionAccum, DEFAULT_MAX_CHUNKS_PER_RESOURCE,
    RRF_K,
};
pub use index::{
    upsert_page, Backlink, BacklinkKind, ChunkSearchHit, EmbedPendingStats, HybridSearchHit,
    ParserStatus, RebuildStats, ResourceMetadata, SearchHit, WorkspaceIndex, MAX_INDEX_TEXT_BYTES,
};
pub use lattice_core::{
    build_link_repair_plan, LinkOccurrence, LinkRepairCandidate, LinkRepairPlan, LinkRepairSource,
    LinkRepairStatus,
};
pub use provenance::{ExportPolicy, SearchProvenance};
pub use semantic::embedding_input_hash;
pub use vector::{
    remove_vector, search_vectors, upsert_vector, SqliteExactScanVectorIndex, VectorCandidate,
    VectorIndex, VectorIndexError,
};
