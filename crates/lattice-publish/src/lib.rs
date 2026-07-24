//! Static HTML export for Lattice pages, interfaces, and artifacts.
//!
//! Exports are self-contained offline snapshots: Markdown pages become HTML,
//! interface dashboards freeze binding query results into JSON + an HTML shell,
//! and artifact packages are copied with an injected read-only binding snapshot.
//! Local assets, chart specs, and attachments referenced by the target are copied
//! into `deps/` with rewritten links; missing dependencies are listed on the
//! export report (required assets fail the export). The live DOM is never scraped.

mod deps;
mod error;
mod export;
mod markdown;
mod snapshot;
mod theme;

pub use deps::{CopiedDependency, DependencyKind, MissingDependency};
pub use error::{Error, Result};
pub use export::{export, ExportReport, ExportTarget};
pub use markdown::{
    collect_markdown_local_refs, markdown_to_html, markdown_to_html_with_rewrites, MarkdownLocalRef,
};
