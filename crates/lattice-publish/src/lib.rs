//! Static HTML export for Lattice pages, interfaces, and artifacts.
//!
//! Exports are self-contained offline snapshots: Markdown pages become HTML,
//! interface dashboards freeze binding query results into JSON + an HTML shell,
//! and artifact packages are copied with an injected read-only binding snapshot.
//! The live DOM is never scraped.

mod error;
mod export;
mod markdown;
mod snapshot;
mod theme;

pub use error::{Error, Result};
pub use export::{export, ExportReport, ExportTarget};
