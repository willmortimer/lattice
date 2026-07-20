use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use lattice_core::{
    inspect_resource, read_resource_range, Resource, ResourceEncoding, ResourceFormatProfile,
    MAX_RESOURCE_RANGE_BYTES,
};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::extract::{
    extract_structured_paths, parse_page, ExtractedLink, StructuredFormat, StructuredPath,
};
use crate::provenance::{ExportPolicy, Sensitivity};
use crate::types::{ParserStatus, ResourceMetadata};

/// Maximum canonical text prefix retained by the derived index.
pub const MAX_INDEX_TEXT_BYTES: u64 = 2 * 1024 * 1024;

pub(crate) struct IndexedRecord {
    pub metadata: ResourceMetadata,
    pub title: String,
    pub headings: String,
    pub body: String,
    pub links: Vec<ExtractedLink>,
    pub tags: Vec<String>,
    pub structured_paths: Vec<StructuredPath>,
    pub text_truncated: bool,
    pub chunk_text: String,
    pub chunk_text_base_byte: usize,
    pub sensitivity: String,
    pub export_policy: String,
}

pub(crate) fn record_from_resource(
    workspace_root: &Path,
    resource: &Resource,
    cancel: &AtomicBool,
) -> Result<IndexedRecord> {
    let rel = crate::paths::normalize_workspace_path(&resource.path)?;
    let inspection = inspect_resource(workspace_root, &rel)?;
    let metadata = ResourceMetadata {
        path: rel.clone(),
        kind: inspection.kind,
        profile: inspection.profile,
        mime: mime_for(&rel, inspection.profile),
        size: inspection.size,
        revision: inspection.revision,
        encoding: inspection.encoding,
        parser_status: ParserStatus::MetadataOnly,
    };

    if inspection.capabilities.is_text {
        let (bytes, truncated) = read_bounded(workspace_root, &rel, inspection.size, cancel)?;
        let text = inspection
            .encoding
            .and_then(|encoding| decode_text(&bytes, encoding));
        Ok(record_from_text(
            metadata,
            text.as_deref(),
            truncated,
            inspection.profile,
        ))
    } else {
        Ok(IndexedRecord {
            metadata,
            title: display_title(&rel),
            headings: String::new(),
            body: String::new(),
            links: Vec::new(),
            tags: Vec::new(),
            structured_paths: Vec::new(),
            text_truncated: false,
            chunk_text: String::new(),
            chunk_text_base_byte: 0,
            sensitivity: Sensitivity::Workspace.as_str().to_string(),
            export_policy: ExportPolicy::Ask.as_str().to_string(),
        })
    }
}

pub(crate) fn record_from_page(path: PathBuf, content: &str) -> Result<IndexedRecord> {
    let (bounded_content, truncated) = bounded_text_prefix(content);
    let metadata = ResourceMetadata {
        path,
        kind: lattice_core::ResourceKind::Page,
        profile: ResourceFormatProfile::Markdown,
        mime: Some("text/markdown".to_string()),
        size: content.len() as u64,
        revision: content_hash(content),
        encoding: Some(ResourceEncoding::Utf8),
        parser_status: ParserStatus::Extracted,
    };
    Ok(record_from_text(
        metadata,
        Some(bounded_content),
        truncated,
        ResourceFormatProfile::Markdown,
    ))
}

pub(crate) fn record_from_text(
    mut metadata: ResourceMetadata,
    text: Option<&str>,
    truncated: bool,
    profile: ResourceFormatProfile,
) -> IndexedRecord {
    let Some(text) = text else {
        let title = display_title(&metadata.path);
        metadata.parser_status = ParserStatus::InvalidEncoding;
        return IndexedRecord {
            metadata,
            title,
            headings: String::new(),
            body: String::new(),
            links: Vec::new(),
            tags: Vec::new(),
            structured_paths: Vec::new(),
            text_truncated: truncated,
            chunk_text: String::new(),
            chunk_text_base_byte: 0,
            sensitivity: Sensitivity::Workspace.as_str().to_string(),
            export_policy: ExportPolicy::Ask.as_str().to_string(),
        };
    };

    let mut title = display_title(&metadata.path);
    let mut headings = String::new();
    let mut body = text.to_string();
    let mut links = Vec::new();
    let mut tags = Vec::new();
    let mut structured_paths = Vec::new();
    let mut chunk_text = text.to_string();
    let mut chunk_text_base_byte = 0;
    let mut sensitivity = Sensitivity::Workspace.as_str().to_string();
    let mut export_policy = ExportPolicy::Ask.as_str().to_string();
    let mut status = if truncated {
        ParserStatus::Truncated
    } else {
        ParserStatus::Extracted
    };

    if profile == ResourceFormatProfile::Markdown {
        let page = parse_page(&metadata.path, text);
        title = page.title;
        headings = page
            .headings
            .iter()
            .map(|heading| format!("{}\t{}\t{}", heading.level, heading.text, heading.line))
            .collect::<Vec<_>>()
            .join("\n");
        body = page.body.clone();
        links = page.links;
        tags = page.tags;
        chunk_text = page.body;
        chunk_text_base_byte = page.body_start_byte;
        if let Some(value) = page.sensitivity {
            sensitivity = Sensitivity::parse(&value).as_str().to_string();
        }
        if let Some(value) = page.export_policy {
            export_policy = ExportPolicy::parse(&value).as_str().to_string();
        }
    }

    if !truncated
        && matches!(
            profile,
            ResourceFormatProfile::Json
                | ResourceFormatProfile::JsonCanvas
                | ResourceFormatProfile::Yaml
        )
    {
        let format = if profile == ResourceFormatProfile::Yaml {
            StructuredFormat::Yaml
        } else {
            StructuredFormat::Json
        };
        let extraction = extract_structured_paths(text, format);
        structured_paths = extraction.paths;
        if !extraction.valid {
            status = ParserStatus::InvalidStructure;
        } else if extraction.truncated {
            status = ParserStatus::Truncated;
        }
    }
    metadata.parser_status = status;
    IndexedRecord {
        metadata,
        title,
        headings,
        body,
        links,
        tags,
        structured_paths,
        text_truncated: truncated,
        chunk_text,
        chunk_text_base_byte,
        sensitivity,
        export_policy,
    }
}

pub(crate) fn parse_headings_for_record(headings: &str) -> Vec<(u8, String, usize)> {
    headings
        .lines()
        .filter_map(|line| {
            let (level, rest) = line.split_once('\t')?;
            let (text, line) = rest.rsplit_once('\t')?;
            Some((level.parse().ok()?, text.to_string(), line.parse().ok()?))
        })
        .collect()
}

pub(crate) fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(crate::error::Error::Cancelled)
    } else {
        Ok(())
    }
}

fn read_bounded(
    root: &Path,
    path: &Path,
    size: u64,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, bool)> {
    let target = size.min(MAX_INDEX_TEXT_BYTES);
    let mut bytes = Vec::with_capacity(target as usize);
    let mut offset = 0;
    while offset < target {
        check_cancel(cancel)?;
        let length = (target - offset).min(MAX_RESOURCE_RANGE_BYTES);
        let range = read_resource_range(root, path, offset, length)?;
        if range.bytes.is_empty() {
            break;
        }
        offset += range.bytes.len() as u64;
        bytes.extend_from_slice(&range.bytes);
    }
    Ok((bytes, size > MAX_INDEX_TEXT_BYTES))
}

fn bounded_text_prefix(content: &str) -> (&str, bool) {
    if content.len() as u64 <= MAX_INDEX_TEXT_BYTES {
        return (content, false);
    }
    let mut end = MAX_INDEX_TEXT_BYTES as usize;
    while !content.is_char_boundary(end) {
        end -= 1;
    }
    (&content[..end], true)
}

fn decode_text(bytes: &[u8], encoding: ResourceEncoding) -> Option<String> {
    match encoding {
        ResourceEncoding::Utf8 => std::str::from_utf8(bytes).ok().map(str::to_owned),
        ResourceEncoding::Utf8Bom => std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf])?)
            .ok()
            .map(str::to_owned),
        ResourceEncoding::Utf16Le | ResourceEncoding::Utf16Be => {
            let bytes = if matches!(encoding, ResourceEncoding::Utf16Le) {
                bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes)
            } else {
                bytes.strip_prefix(&[0xfe, 0xff]).unwrap_or(bytes)
            };
            if bytes.len() % 2 != 0 {
                return None;
            }
            let units = bytes
                .chunks_exact(2)
                .map(|pair| {
                    if encoding == ResourceEncoding::Utf16Le {
                        u16::from_le_bytes([pair[0], pair[1]])
                    } else {
                        u16::from_be_bytes([pair[0], pair[1]])
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16(&units).ok()
        }
    }
}

fn mime_for(path: &Path, profile: ResourceFormatProfile) -> Option<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    let mime = match profile {
        ResourceFormatProfile::Markdown => "text/markdown",
        ResourceFormatProfile::JsonCanvas | ResourceFormatProfile::Json => "application/json",
        ResourceFormatProfile::Yaml => "application/yaml",
        ResourceFormatProfile::PlainText => match extension.as_str() {
            "csv" => "text/csv",
            "tsv" => "text/tab-separated-values",
            "log" => "text/plain",
            _ => "text/plain",
        },
        ResourceFormatProfile::Code => match extension.as_str() {
            "rs" => "text/rust",
            "ts" | "tsx" => "text/typescript",
            "js" | "jsx" => "text/javascript",
            "html" | "htm" => "text/html",
            "css" | "scss" => "text/css",
            "py" => "text/x-python",
            "sql" => "application/sql",
            _ => "text/plain",
        },
        ResourceFormatProfile::Image => match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "avif" => "image/avif",
            "svg" => "image/svg+xml",
            "tif" | "tiff" => "image/tiff",
            _ => return None,
        },
        ResourceFormatProfile::Pdf => "application/pdf",
        ResourceFormatProfile::SqliteDataApp => "application/vnd.sqlite3",
        ResourceFormatProfile::UnknownBinary | ResourceFormatProfile::UnknownDirectory => {
            return None
        }
    };
    Some(mime.to_string())
}

fn display_title(path: &Path) -> String {
    path.file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Untitled".to_string())
}

fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}
