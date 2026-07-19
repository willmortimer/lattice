use std::path::PathBuf;

use lattice_core::{ResourceEncoding, ResourceFormatProfile, ResourceKind};
use serde::Serialize;

/// Statistics from a full index rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildStats {
    pub resources_indexed: usize,
    pub resources_removed: usize,
    pub pages_indexed: usize,
    pub pages_removed: usize,
}

/// Parser state stored with each indexed resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParserStatus {
    MetadataOnly,
    Extracted,
    Truncated,
    InvalidEncoding,
    InvalidStructure,
}

/// Metadata and bounded parser state for one generic resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetadata {
    pub path: PathBuf,
    pub kind: ResourceKind,
    pub profile: ResourceFormatProfile,
    pub mime: Option<String>,
    pub size: u64,
    pub revision: String,
    pub encoding: Option<ResourceEncoding>,
    pub parser_status: ParserStatus,
}

/// One full-text search hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    pub path: PathBuf,
    pub title: String,
    pub snippet: Option<String>,
    pub rank: f64,
}

/// One structural chunk search hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSearchHit {
    pub path: PathBuf,
    pub title: String,
    pub chunk_id: String,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
    pub snippet: Option<String>,
    pub rank: f64,
}

/// A resource that links to a target path, including a repairable source span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backlink {
    pub source_path: PathBuf,
    pub kind: BacklinkKind,
    pub target: String,
    pub anchor: Option<String>,
    pub label: Option<String>,
    pub source_start_byte: Option<usize>,
    pub source_end_byte: Option<usize>,
    pub source_start_line: Option<usize>,
    pub source_start_column: Option<usize>,
    pub source_end_line: Option<usize>,
    pub source_end_column: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BacklinkKind {
    Wiki,
    Md,
}
