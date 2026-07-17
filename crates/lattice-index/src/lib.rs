//! Derived workspace search index for Lattice.
//!
//! The index lives at `<workspace>/.lattice/index.sqlite` and is fully
//! rebuildable from generic resources on disk. Text extraction is deliberately
//! bounded; binary resources retain searchable names and runtime metadata.

mod error;
mod extract;
mod index;

pub use error::{Error, Result};
pub use extract::{
    extract_structured_paths, parse_page, ExtractedLink, Heading, LinkKind, PageIndexData,
    StructuredExtraction, StructuredFormat, StructuredPath,
};
pub use index::{
    upsert_page, Backlink, BacklinkKind, ParserStatus, RebuildStats, ResourceMetadata, SearchHit,
    WorkspaceIndex, MAX_INDEX_TEXT_BYTES,
};
pub use lattice_core::{
    build_link_repair_plan, LinkOccurrence, LinkRepairCandidate, LinkRepairPlan,
    LinkRepairSource, LinkRepairStatus,
};
