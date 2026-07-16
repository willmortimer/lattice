//! Derived workspace search index for Lattice.
//!
//! The index lives at `<workspace>/.lattice/index.sqlite` and is fully
//! rebuildable from Markdown pages on disk. WS5 will call [`upsert_page`]
//! after command-engine writes; WS6 will consume [`WorkspaceIndex::search`]
//! and [`WorkspaceIndex::backlinks`] from the desktop shell.

mod error;
mod extract;
mod index;

pub use error::{Error, Result};
pub use extract::{parse_page, ExtractedLink, Heading, LinkKind, PageIndexData};
pub use index::{upsert_page, Backlink, BacklinkKind, RebuildStats, SearchHit, WorkspaceIndex};
