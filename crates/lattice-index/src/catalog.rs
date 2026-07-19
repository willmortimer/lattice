use std::path::PathBuf;

use lattice_core::{ResourceEncoding, ResourceFormatProfile, ResourceKind};
use rusqlite::Row;

use crate::types::ParserStatus;

pub(crate) fn metadata_from_row(row: &Row<'_>) -> rusqlite::Result<crate::types::ResourceMetadata> {
    Ok(crate::types::ResourceMetadata {
        path: PathBuf::from(row.get::<_, String>(0)?),
        kind: kind_from_db(&row.get::<_, String>(1)?),
        profile: profile_from_db(&row.get::<_, String>(2)?),
        mime: row.get(3)?,
        size: row.get::<_, i64>(4)? as u64,
        revision: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        encoding: row.get::<_, Option<String>>(6)?.and_then(encoding_from_db),
        parser_status: parser_status_from_db(&row.get::<_, String>(7)?),
    })
}

pub(crate) fn kind_db(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Page => "page",
        ResourceKind::Canvas => "canvas",
        ResourceKind::DataApp => "data-app",
        ResourceKind::Dataset => "dataset",
        ResourceKind::Notebook => "notebook",
        ResourceKind::Ink => "ink",
        ResourceKind::Artifact => "artifact",
        ResourceKind::App => "app",
        ResourceKind::Workflow => "workflow",
        ResourceKind::Task => "task",
        ResourceKind::Folder => "folder",
        ResourceKind::File => "file",
    }
}

pub(crate) fn kind_from_db(value: &str) -> ResourceKind {
    match value {
        "canvas" => ResourceKind::Canvas,
        "data-app" => ResourceKind::DataApp,
        "dataset" => ResourceKind::Dataset,
        "notebook" => ResourceKind::Notebook,
        "ink" => ResourceKind::Ink,
        "artifact" => ResourceKind::Artifact,
        "app" => ResourceKind::App,
        "workflow" => ResourceKind::Workflow,
        "task" => ResourceKind::Task,
        "folder" => ResourceKind::Folder,
        "file" => ResourceKind::File,
        _ => ResourceKind::Page,
    }
}

pub(crate) fn profile_db(profile: ResourceFormatProfile) -> &'static str {
    match profile {
        ResourceFormatProfile::Markdown => "markdown",
        ResourceFormatProfile::JsonCanvas => "json-canvas",
        ResourceFormatProfile::SqliteDataApp => "sqlite-data-app",
        ResourceFormatProfile::Image => "image",
        ResourceFormatProfile::Pdf => "pdf",
        ResourceFormatProfile::PlainText => "plain-text",
        ResourceFormatProfile::Code => "code",
        ResourceFormatProfile::Json => "json",
        ResourceFormatProfile::Yaml => "yaml",
        ResourceFormatProfile::UnknownBinary => "unknown-binary",
        ResourceFormatProfile::UnknownDirectory => "unknown-directory",
    }
}

pub(crate) fn profile_from_db(value: &str) -> ResourceFormatProfile {
    match value {
        "json-canvas" => ResourceFormatProfile::JsonCanvas,
        "sqlite-data-app" => ResourceFormatProfile::SqliteDataApp,
        "image" => ResourceFormatProfile::Image,
        "pdf" => ResourceFormatProfile::Pdf,
        "plain-text" => ResourceFormatProfile::PlainText,
        "code" => ResourceFormatProfile::Code,
        "json" => ResourceFormatProfile::Json,
        "yaml" => ResourceFormatProfile::Yaml,
        "unknown-binary" => ResourceFormatProfile::UnknownBinary,
        "unknown-directory" => ResourceFormatProfile::UnknownDirectory,
        _ => ResourceFormatProfile::Markdown,
    }
}

pub(crate) fn encoding_db(encoding: ResourceEncoding) -> &'static str {
    match encoding {
        ResourceEncoding::Utf8 => "utf8",
        ResourceEncoding::Utf8Bom => "utf8-bom",
        ResourceEncoding::Utf16Le => "utf16-le",
        ResourceEncoding::Utf16Be => "utf16-be",
    }
}

pub(crate) fn encoding_from_db(value: String) -> Option<ResourceEncoding> {
    Some(match value.as_str() {
        "utf8" => ResourceEncoding::Utf8,
        "utf8-bom" => ResourceEncoding::Utf8Bom,
        "utf16-le" => ResourceEncoding::Utf16Le,
        "utf16-be" => ResourceEncoding::Utf16Be,
        _ => return None,
    })
}

pub(crate) fn parser_status_db(status: ParserStatus) -> &'static str {
    match status {
        ParserStatus::MetadataOnly => "metadata-only",
        ParserStatus::Extracted => "extracted",
        ParserStatus::Truncated => "truncated",
        ParserStatus::InvalidEncoding => "invalid-encoding",
        ParserStatus::InvalidStructure => "invalid-structure",
    }
}

pub(crate) fn parser_status_from_db(value: &str) -> ParserStatus {
    match value {
        "extracted" => ParserStatus::Extracted,
        "truncated" => ParserStatus::Truncated,
        "invalid-encoding" => ParserStatus::InvalidEncoding,
        "invalid-structure" => ParserStatus::InvalidStructure,
        _ => ParserStatus::MetadataOnly,
    }
}
