//! Bounded native resource inspection and read APIs.
//!
//! This module deliberately keeps [`ResourceKind`](crate::ResourceKind)
//! coarse. A resource's format profile is derived from its path, bounded
//! probe, and (where useful) lightweight validation. Inspection never writes
//! to the workspace and all filesystem access is containment checked after
//! symlink resolution.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ResourceKind, Severity};

/// Maximum number of bytes used for format recognition and validation.
pub const MAX_FORMAT_PROBE_BYTES: u64 = 64 * 1024;
/// Maximum number of bytes returned by one raw range read.
pub const MAX_RESOURCE_RANGE_BYTES: u64 = 1024 * 1024;
/// Default maximum size of one semantic resource edit.
pub const DEFAULT_RESOURCE_EDIT_BYTES: u64 = 10 * 1024 * 1024;

/// The more specific format profile derived for a coarse resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceFormatProfile {
    Markdown,
    JsonCanvas,
    SqliteDataApp,
    Image,
    Pdf,
    PlainText,
    Code,
    Json,
    Yaml,
    UnknownBinary,
}

/// Capabilities exposed by a recognized format profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatCapabilities {
    pub can_inspect: bool,
    pub can_read_range: bool,
    pub can_read_text_window: bool,
    pub can_update: bool,
    pub is_text: bool,
    pub is_binary: bool,
    pub validates_structure: bool,
    pub max_edit_bytes: u64,
}

impl FormatCapabilities {
    fn text(validates_structure: bool) -> Self {
        Self {
            can_inspect: true,
            can_read_range: true,
            can_read_text_window: true,
            can_update: true,
            is_text: true,
            is_binary: false,
            validates_structure,
            max_edit_bytes: DEFAULT_RESOURCE_EDIT_BYTES,
        }
    }

    fn binary() -> Self {
        Self {
            can_inspect: true,
            can_read_range: true,
            can_read_text_window: false,
            can_update: true,
            is_text: false,
            is_binary: true,
            validates_structure: false,
            max_edit_bytes: DEFAULT_RESOURCE_EDIT_BYTES,
        }
    }

    fn package() -> Self {
        Self {
            can_inspect: true,
            can_read_range: false,
            can_read_text_window: false,
            can_update: false,
            is_text: false,
            is_binary: true,
            validates_structure: false,
            max_edit_bytes: DEFAULT_RESOURCE_EDIT_BYTES,
        }
    }
}

impl ResourceFormatProfile {
    pub fn capabilities(self) -> FormatCapabilities {
        match self {
            Self::Markdown | Self::PlainText | Self::Code => FormatCapabilities::text(false),
            Self::JsonCanvas | Self::Json | Self::Yaml => FormatCapabilities::text(true),
            Self::SqliteDataApp => FormatCapabilities::binary(),
            Self::Image | Self::Pdf | Self::UnknownBinary => FormatCapabilities::binary(),
        }
    }
}

/// A diagnostic produced without changing canonical content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDiagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

/// Text encoding recognized by the bounded probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

/// Native resource metadata plus derived format information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInspection {
    pub path: PathBuf,
    pub kind: ResourceKind,
    pub profile: ResourceFormatProfile,
    pub capabilities: FormatCapabilities,
    pub revision: String,
    pub size: u64,
    pub is_directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<ResourceEncoding>,
    pub probe_bytes: u64,
    pub diagnostics: Vec<ResourceDiagnostic>,
}

/// A bounded raw read result. The native Tauri command returns the bytes as a
/// raw IPC response; this contract is used by headless Rust callers and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRange {
    pub path: PathBuf,
    pub offset: u64,
    pub requested_length: u64,
    pub bytes: Vec<u8>,
    pub total_size: u64,
}

/// A bounded decoded text window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextWindow {
    pub path: PathBuf,
    pub offset: u64,
    pub requested_length: u64,
    pub bytes_read: u64,
    pub total_size: u64,
    pub truncated: bool,
    pub encoding: ResourceEncoding,
    pub content: String,
}

/// Structured errors shared by native resource APIs and Tauri adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ResourceRuntimeError {
    #[error("invalid workspace root {root}: {message}")]
    InvalidWorkspaceRoot { root: PathBuf, message: String },
    #[error("resource path {path} escapes the workspace root")]
    OutsideWorkspace { path: PathBuf },
    #[error("resource {path} was not found")]
    NotFound { path: PathBuf },
    #[error("resource {path} is a directory, not a file")]
    NotAFile { path: PathBuf },
    #[error("filesystem error at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("range at {path} has invalid offset {offset} for size {size}")]
    InvalidOffset {
        path: PathBuf,
        offset: u64,
        size: u64,
    },
    #[error("range length {length} exceeds the {max} byte limit")]
    RangeTooLarge { length: u64, max: u64 },
    #[error("text window at {path} has length {length} above the {max} byte limit")]
    TextWindowTooLarge {
        path: PathBuf,
        length: u64,
        max: u64,
    },
    #[error("resource {path} is not valid text")]
    InvalidEncoding { path: PathBuf },
    #[error("resource {path} is binary and has no text window")]
    BinaryResource { path: PathBuf },
    #[error("UTF-16 text window at {path} must use even byte offsets and lengths")]
    MisalignedUtf16Window { path: PathBuf },
}

/// The built-in registry for the initial native resource profiles.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuiltinFormatRegistry;

impl BuiltinFormatRegistry {
    pub const fn new() -> Self {
        Self
    }

    pub fn profiles(&self) -> &'static [ResourceFormatProfile] {
        &[
            ResourceFormatProfile::Markdown,
            ResourceFormatProfile::JsonCanvas,
            ResourceFormatProfile::SqliteDataApp,
            ResourceFormatProfile::Image,
            ResourceFormatProfile::Pdf,
            ResourceFormatProfile::PlainText,
            ResourceFormatProfile::Code,
            ResourceFormatProfile::Json,
            ResourceFormatProfile::Yaml,
            ResourceFormatProfile::UnknownBinary,
        ]
    }
}

/// Inspect one workspace-relative resource using a max-64 KiB probe.
pub fn inspect_resource(
    root: &Path,
    relative_path: &Path,
) -> Result<ResourceInspection, ResourceRuntimeError> {
    let (normalized, absolute) = resolve_contained(root, relative_path)?;
    let metadata = std::fs::metadata(&absolute).map_err(|source| io_error(&normalized, source))?;
    let size = metadata.len();
    let kind = ResourceKind::classify(&absolute, metadata.is_dir());
    let revision = content_revision(&absolute, metadata.is_dir(), size)?;

    if metadata.is_dir() {
        return Ok(ResourceInspection {
            path: normalized,
            kind,
            profile: ResourceFormatProfile::SqliteDataApp,
            capabilities: FormatCapabilities::package(),
            revision,
            size,
            is_directory: true,
            encoding: None,
            probe_bytes: 0,
            diagnostics: Vec::new(),
        });
    }

    let probe = read_probe(&absolute, size)?;
    let (profile, diagnostics, encoding) = recognize(&normalized, kind, size, &probe);
    Ok(ResourceInspection {
        path: normalized,
        kind,
        profile,
        capabilities: profile.capabilities(),
        revision,
        size,
        is_directory: false,
        encoding,
        probe_bytes: probe.len() as u64,
        diagnostics,
    })
}

/// Read at most [`MAX_RESOURCE_RANGE_BYTES`] from a contained file.
pub fn read_resource_range(
    root: &Path,
    relative_path: &Path,
    offset: u64,
    length: u64,
) -> Result<ResourceRange, ResourceRuntimeError> {
    let (normalized, absolute) = resolve_contained(root, relative_path)?;
    let metadata = std::fs::metadata(&absolute).map_err(|source| io_error(&normalized, source))?;
    if metadata.is_dir() {
        return Err(ResourceRuntimeError::NotAFile { path: normalized });
    }
    if length > MAX_RESOURCE_RANGE_BYTES {
        return Err(ResourceRuntimeError::RangeTooLarge {
            length,
            max: MAX_RESOURCE_RANGE_BYTES,
        });
    }
    if offset > metadata.len() {
        return Err(ResourceRuntimeError::InvalidOffset {
            path: normalized,
            offset,
            size: metadata.len(),
        });
    }
    let bytes = read_at(&absolute, offset, length as usize)
        .map_err(|source| io_error(relative_path, source))?;
    Ok(ResourceRange {
        path: normalized,
        offset,
        requested_length: length,
        bytes,
        total_size: metadata.len(),
    })
}

/// Read and decode a bounded text window from a contained text resource.
pub fn read_text_window(
    root: &Path,
    relative_path: &Path,
    offset: u64,
    length: u64,
) -> Result<TextWindow, ResourceRuntimeError> {
    let inspection = inspect_resource(root, relative_path)?;
    let normalized = inspection.path.clone();
    if !inspection.capabilities.can_read_text_window {
        return Err(ResourceRuntimeError::BinaryResource { path: normalized });
    }
    let encoding = inspection
        .encoding
        .ok_or_else(|| ResourceRuntimeError::InvalidEncoding {
            path: inspection.path.clone(),
        })?;
    if length > MAX_RESOURCE_RANGE_BYTES {
        return Err(ResourceRuntimeError::TextWindowTooLarge {
            path: inspection.path,
            length,
            max: MAX_RESOURCE_RANGE_BYTES,
        });
    }
    if offset > inspection.size {
        return Err(ResourceRuntimeError::InvalidOffset {
            path: normalized,
            offset,
            size: inspection.size,
        });
    }
    if matches!(
        encoding,
        ResourceEncoding::Utf16Le | ResourceEncoding::Utf16Be
    ) && (offset % 2 != 0 || length % 2 != 0)
    {
        return Err(ResourceRuntimeError::MisalignedUtf16Window { path: normalized });
    }

    let (_, absolute) = resolve_contained(root, relative_path)?;
    let bytes = read_at(&absolute, offset, length as usize)
        .map_err(|source| io_error(relative_path, source))?;
    let content =
        decode_text(&bytes, encoding).map_err(|_| ResourceRuntimeError::InvalidEncoding {
            path: normalized.clone(),
        })?;
    let bytes_read = bytes.len() as u64;
    Ok(TextWindow {
        path: normalized,
        offset,
        requested_length: length,
        bytes_read,
        total_size: inspection.size,
        truncated: offset.saturating_add(bytes_read) < inspection.size,
        encoding,
        content,
    })
}

fn resolve_contained(
    root: &Path,
    relative: &Path,
) -> Result<(PathBuf, PathBuf), ResourceRuntimeError> {
    let normalized = normalize_relative(relative)?;
    let canonical_root =
        root.canonicalize()
            .map_err(|source| ResourceRuntimeError::InvalidWorkspaceRoot {
                root: root.to_path_buf(),
                message: source.to_string(),
            })?;
    let candidate = canonical_root.join(&normalized);
    let canonical_candidate = candidate.canonicalize().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            ResourceRuntimeError::NotFound {
                path: normalized.clone(),
            }
        } else {
            io_error(&normalized, source)
        }
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ResourceRuntimeError::OutsideWorkspace { path: normalized });
    }
    Ok((normalized, canonical_candidate))
}

fn normalize_relative(path: &Path) -> Result<PathBuf, ResourceRuntimeError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ResourceRuntimeError::OutsideWorkspace {
                    path: path.to_path_buf(),
                })
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(ResourceRuntimeError::NotFound { path: normalized });
    }
    Ok(normalized)
}

fn io_error(path: &Path, source: std::io::Error) -> ResourceRuntimeError {
    ResourceRuntimeError::Io {
        path: path.to_path_buf(),
        message: source.to_string(),
    }
}

fn content_revision(
    path: &Path,
    is_directory: bool,
    size: u64,
) -> Result<String, ResourceRuntimeError> {
    if !is_directory {
        let file = File::open(path).map_err(|source| io_error(path, source))?;
        return lattice_storage::sha256_reader(file).map_err(|source| io_error(path, source));
    }
    let _ = size;
    lattice_storage::sha256_reader(std::io::empty()).map_err(|source| io_error(path, source))
}

fn read_probe(path: &Path, size: u64) -> Result<Vec<u8>, ResourceRuntimeError> {
    read_at(path, 0, size.min(MAX_FORMAT_PROBE_BYTES) as usize)
        .map_err(|source| io_error(path, source))
}

fn read_at(path: &Path, offset: u64, length: usize) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0u8; length];
    let mut read = 0;
    while read < length {
        let count = file.read(&mut bytes[read..])?;
        if count == 0 {
            break;
        }
        read += count;
    }
    bytes.truncate(read);
    Ok(bytes)
}

fn recognize(
    path: &Path,
    kind: ResourceKind,
    size: u64,
    probe: &[u8],
) -> (
    ResourceFormatProfile,
    Vec<ResourceDiagnostic>,
    Option<ResourceEncoding>,
) {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let extension_profile = extension.as_deref().and_then(profile_for_extension);
    let magic_profile = profile_for_magic(probe);
    let mut profile = extension_profile
        .or(magic_profile)
        .unwrap_or(ResourceFormatProfile::UnknownBinary);
    let mut diagnostics = Vec::new();
    let mut encoding = None;

    if kind == ResourceKind::DataApp
        || extension_profile == Some(ResourceFormatProfile::SqliteDataApp)
    {
        profile = ResourceFormatProfile::SqliteDataApp;
    }

    if let Some(expected) = extension_profile {
        if matches!(
            expected,
            ResourceFormatProfile::Pdf | ResourceFormatProfile::Image
        ) && magic_profile != Some(expected)
        {
            diagnostics.push(diagnostic(
                "magic-mismatch",
                Severity::Error,
                format!(
                    "extension suggests {expected:?}, but the bounded probe has no matching magic"
                ),
            ));
        }
    }

    let text_profile = matches!(
        profile,
        ResourceFormatProfile::Markdown
            | ResourceFormatProfile::JsonCanvas
            | ResourceFormatProfile::PlainText
            | ResourceFormatProfile::Code
            | ResourceFormatProfile::Json
            | ResourceFormatProfile::Yaml
    );
    if text_profile {
        match detect_encoding(probe) {
            Ok(found) => {
                encoding = Some(found);
                if size > MAX_FORMAT_PROBE_BYTES
                    && matches!(
                        profile,
                        ResourceFormatProfile::JsonCanvas
                            | ResourceFormatProfile::Json
                            | ResourceFormatProfile::Yaml
                    )
                {
                    diagnostics.push(diagnostic(
                        "probe-truncated",
                        Severity::Warning,
                        "structured validation was limited to the 64 KiB probe",
                    ));
                } else if size <= MAX_FORMAT_PROBE_BYTES {
                    let text = decode_text(probe, found).unwrap_or_default();
                    if matches!(
                        profile,
                        ResourceFormatProfile::Json | ResourceFormatProfile::JsonCanvas
                    ) && !valid_json_document(&text)
                    {
                        diagnostics.push(diagnostic(
                            if profile == ResourceFormatProfile::JsonCanvas {
                                "invalid-json-canvas"
                            } else {
                                "invalid-json"
                            },
                            Severity::Error,
                            "the complete bounded resource is not valid JSON",
                        ));
                    } else if profile == ResourceFormatProfile::Yaml
                        && serde_yaml::from_str::<serde_yaml::Value>(&text).is_err()
                    {
                        diagnostics.push(diagnostic(
                            "invalid-yaml",
                            Severity::Error,
                            "the complete bounded resource is not valid YAML",
                        ));
                    }
                }
            }
            Err(_) => {
                diagnostics.push(diagnostic(
                    "invalid-encoding",
                    Severity::Error,
                    "the bounded probe is not valid UTF-8 or UTF-16 text",
                ));
            }
        }
    } else if profile == ResourceFormatProfile::UnknownBinary && size == 0 {
        profile = ResourceFormatProfile::PlainText;
        encoding = Some(ResourceEncoding::Utf8);
    }

    (profile, diagnostics, encoding)
}

fn profile_for_extension(extension: &str) -> Option<ResourceFormatProfile> {
    Some(match extension {
        "md" | "markdown" => ResourceFormatProfile::Markdown,
        "canvas" => ResourceFormatProfile::JsonCanvas,
        "sqlite" | "sqlite3" | "db" => ResourceFormatProfile::SqliteDataApp,
        "pdf" => ResourceFormatProfile::Pdf,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "tif" | "tiff" | "bmp" | "svg" => {
            ResourceFormatProfile::Image
        }
        "json" => ResourceFormatProfile::Json,
        "yaml" | "yml" => ResourceFormatProfile::Yaml,
        "txt" | "text" | "log" | "csv" | "tsv" => ResourceFormatProfile::PlainText,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "css" | "scss" | "html" | "htm" | "toml" | "sh"
        | "bash" | "zsh" | "fish" | "py" | "rb" | "go" | "java" | "kt" | "swift" | "c" | "h"
        | "cpp" | "hpp" | "sql" | "graphql" => ResourceFormatProfile::Code,
        _ => return None,
    })
}

fn profile_for_magic(probe: &[u8]) -> Option<ResourceFormatProfile> {
    if probe.starts_with(b"%PDF-") {
        return Some(ResourceFormatProfile::Pdf);
    }
    if probe.starts_with(b"\x89PNG\r\n\x1a\n")
        || probe.starts_with(&[0xff, 0xd8, 0xff])
        || probe.starts_with(b"GIF87a")
        || probe.starts_with(b"GIF89a")
        || (probe.starts_with(b"RIFF") && probe.get(8..12) == Some(b"WEBP"))
        || probe.starts_with(b"II*\0")
        || probe.starts_with(b"MM\0*")
    {
        return Some(ResourceFormatProfile::Image);
    }
    if let Ok(encoding) = detect_encoding(probe) {
        if let Ok(text) = decode_text(probe, encoding) {
            let trimmed = text.trim_start();
            if trimmed.starts_with("<svg")
                || trimmed.starts_with("<?xml") && trimmed.contains("<svg")
            {
                return Some(ResourceFormatProfile::Image);
            }
        }
    }
    None
}

fn detect_encoding(bytes: &[u8]) -> Result<ResourceEncoding, ()> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        std::str::from_utf8(&bytes[3..]).map_err(|_| ())?;
        return Ok(ResourceEncoding::Utf8Bom);
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        decode_utf16(&bytes[2..], true).map_err(|_| ())?;
        return Ok(ResourceEncoding::Utf16Le);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        decode_utf16(&bytes[2..], false).map_err(|_| ())?;
        return Ok(ResourceEncoding::Utf16Be);
    }
    std::str::from_utf8(bytes).map_err(|_| ())?;
    Ok(ResourceEncoding::Utf8)
}

fn decode_text(bytes: &[u8], encoding: ResourceEncoding) -> Result<String, ()> {
    match encoding {
        ResourceEncoding::Utf8 => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| ()),
        ResourceEncoding::Utf8Bom => {
            std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
                .map(str::to_owned)
                .map_err(|_| ())
        }
        ResourceEncoding::Utf16Le => decode_utf16(bytes, true),
        ResourceEncoding::Utf16Be => decode_utf16(bytes, false),
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, ()> {
    if bytes.len() % 2 != 0 {
        return Err(());
    }
    let units = bytes
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| ())
}

fn valid_json_document(text: &str) -> bool {
    struct Parser<'a> {
        bytes: &'a [u8],
        position: usize,
    }

    impl<'a> Parser<'a> {
        fn whitespace(&mut self) {
            while self
                .bytes
                .get(self.position)
                .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
            {
                self.position += 1;
            }
        }

        fn value(&mut self) -> bool {
            self.whitespace();
            match self.bytes.get(self.position) {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => self.string(),
                Some(b't') => self.literal(b"true"),
                Some(b'f') => self.literal(b"false"),
                Some(b'n') => self.literal(b"null"),
                Some(b'-' | b'0'..=b'9') => self.number(),
                _ => false,
            }
        }

        fn literal(&mut self, literal: &[u8]) -> bool {
            if self.bytes.get(self.position..self.position + literal.len()) == Some(literal) {
                self.position += literal.len();
                true
            } else {
                false
            }
        }

        fn string(&mut self) -> bool {
            if self.bytes.get(self.position) != Some(&b'"') {
                return false;
            }
            self.position += 1;
            while let Some(byte) = self.bytes.get(self.position).copied() {
                self.position += 1;
                match byte {
                    b'"' => return true,
                    b'\\' => {
                        let Some(escaped) = self.bytes.get(self.position).copied() else {
                            return false;
                        };
                        self.position += 1;
                        if escaped == b'u' {
                            if self.position + 4 > self.bytes.len()
                                || !self.bytes[self.position..self.position + 4]
                                    .iter()
                                    .all(|byte| byte.is_ascii_hexdigit())
                            {
                                return false;
                            }
                            self.position += 4;
                        } else if !matches!(
                            escaped,
                            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't'
                        ) {
                            return false;
                        }
                    }
                    byte if byte < 0x20 => return false,
                    _ => {}
                }
            }
            false
        }

        fn number(&mut self) -> bool {
            let start = self.position;
            if self.bytes.get(self.position) == Some(&b'-') {
                self.position += 1;
            }
            match self.bytes.get(self.position) {
                Some(b'0') => self.position += 1,
                Some(b'1'..=b'9') => {
                    self.position += 1;
                    while self
                        .bytes
                        .get(self.position)
                        .is_some_and(u8::is_ascii_digit)
                    {
                        self.position += 1;
                    }
                }
                _ => return false,
            }
            if self.bytes.get(self.position) == Some(&b'.') {
                self.position += 1;
                let digits = self.position;
                while self
                    .bytes
                    .get(self.position)
                    .is_some_and(u8::is_ascii_digit)
                {
                    self.position += 1;
                }
                if digits == self.position {
                    return false;
                }
            }
            if self
                .bytes
                .get(self.position)
                .is_some_and(|byte| matches!(byte, b'e' | b'E'))
            {
                self.position += 1;
                if self
                    .bytes
                    .get(self.position)
                    .is_some_and(|byte| matches!(byte, b'+' | b'-'))
                {
                    self.position += 1;
                }
                let digits = self.position;
                while self
                    .bytes
                    .get(self.position)
                    .is_some_and(u8::is_ascii_digit)
                {
                    self.position += 1;
                }
                if digits == self.position {
                    return false;
                }
            }
            self.position > start
        }

        fn array(&mut self) -> bool {
            self.position += 1;
            self.whitespace();
            if self.bytes.get(self.position) == Some(&b']') {
                self.position += 1;
                return true;
            }
            loop {
                if !self.value() {
                    return false;
                }
                self.whitespace();
                match self.bytes.get(self.position) {
                    Some(b',') => self.position += 1,
                    Some(b']') => {
                        self.position += 1;
                        return true;
                    }
                    _ => return false,
                }
            }
        }

        fn object(&mut self) -> bool {
            self.position += 1;
            self.whitespace();
            if self.bytes.get(self.position) == Some(&b'}') {
                self.position += 1;
                return true;
            }
            loop {
                if !self.string() {
                    return false;
                }
                self.whitespace();
                if self.bytes.get(self.position) != Some(&b':') {
                    return false;
                }
                self.position += 1;
                if !self.value() {
                    return false;
                }
                self.whitespace();
                match self.bytes.get(self.position) {
                    Some(b',') => self.position += 1,
                    Some(b'}') => {
                        self.position += 1;
                        return true;
                    }
                    _ => return false,
                }
            }
        }
    }

    let mut parser = Parser {
        bytes: text.trim().as_bytes(),
        position: 0,
    };
    if !parser.value() {
        return false;
    }
    parser.whitespace();
    parser.position == parser.bytes.len()
}

fn diagnostic(code: &str, severity: Severity, message: impl Into<String>) -> ResourceDiagnostic {
    ResourceDiagnostic {
        code: code.to_string(),
        severity,
        message: message.into(),
        offset: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn workspace() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        crate::Workspace::init(directory.path(), "Runtime tests").unwrap();
        directory
    }

    #[test]
    fn registry_profiles_are_stable_and_complete() {
        let profiles = BuiltinFormatRegistry::new().profiles();
        assert_eq!(profiles.len(), 10);
        assert!(profiles.contains(&ResourceFormatProfile::JsonCanvas));
        assert!(profiles.contains(&ResourceFormatProfile::UnknownBinary));
    }

    #[test]
    fn recognizes_text_structured_and_magic_formats() {
        let directory = workspace();
        std::fs::write(directory.path().join("board.canvas"), br#"{"nodes":[]}"#).unwrap();
        std::fs::write(directory.path().join("data.yaml"), b"items:\n  - one\n").unwrap();
        std::fs::write(directory.path().join("paper.pdf"), b"%PDF-1.7\n").unwrap();
        assert_eq!(
            inspect_resource(directory.path(), Path::new("board.canvas"))
                .unwrap()
                .profile,
            ResourceFormatProfile::JsonCanvas
        );
        assert_eq!(
            inspect_resource(directory.path(), Path::new("data.yaml"))
                .unwrap()
                .profile,
            ResourceFormatProfile::Yaml
        );
        assert_eq!(
            inspect_resource(directory.path(), Path::new("paper.pdf"))
                .unwrap()
                .profile,
            ResourceFormatProfile::Pdf
        );
    }

    #[test]
    fn malformed_json_and_yaml_are_diagnostics() {
        let directory = workspace();
        std::fs::write(directory.path().join("bad.json"), b"{\"value\":}").unwrap();
        std::fs::write(directory.path().join("bad.yaml"), b"items: [one").unwrap();
        let json = inspect_resource(directory.path(), Path::new("bad.json")).unwrap();
        let yaml = inspect_resource(directory.path(), Path::new("bad.yaml")).unwrap();
        assert!(json
            .diagnostics
            .iter()
            .any(|item| item.code == "invalid-json"));
        assert!(yaml
            .diagnostics
            .iter()
            .any(|item| item.code == "invalid-yaml"));
    }

    #[test]
    fn bounded_range_and_text_window_enforce_limits() {
        let directory = workspace();
        std::fs::write(directory.path().join("note.txt"), b"0123456789").unwrap();
        let range = read_resource_range(directory.path(), Path::new("note.txt"), 2, 4).unwrap();
        assert_eq!(range.bytes, b"2345");
        let window = read_text_window(directory.path(), Path::new("note.txt"), 2, 4).unwrap();
        assert_eq!(window.content, "2345");
        assert!(matches!(
            read_resource_range(
                directory.path(),
                Path::new("note.txt"),
                0,
                MAX_RESOURCE_RANGE_BYTES + 1
            ),
            Err(ResourceRuntimeError::RangeTooLarge { .. })
        ));
        assert!(matches!(
            read_resource_range(directory.path(), Path::new("note.txt"), 11, 0),
            Err(ResourceRuntimeError::InvalidOffset { .. })
        ));
    }

    #[test]
    fn recognizes_utf16_and_rejects_binary_text_window() {
        let directory = workspace();
        std::fs::write(
            directory.path().join("utf16.txt"),
            [0xff, 0xfe, b'h', 0, b'i', 0],
        )
        .unwrap();
        std::fs::write(directory.path().join("blob.bin"), [0, 159, 146, 150, 255]).unwrap();
        let inspection = inspect_resource(directory.path(), Path::new("utf16.txt")).unwrap();
        assert_eq!(inspection.encoding, Some(ResourceEncoding::Utf16Le));
        assert_eq!(
            read_text_window(directory.path(), Path::new("utf16.txt"), 2, 4)
                .unwrap()
                .content,
            "hi"
        );
        assert!(matches!(
            read_text_window(directory.path(), Path::new("blob.bin"), 0, 4),
            Err(ResourceRuntimeError::BinaryResource { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn traversal_and_symlink_escape_are_rejected() {
        let directory = workspace();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            directory.path().join("link.txt"),
        )
        .unwrap();
        assert!(matches!(
            inspect_resource(directory.path(), Path::new("../secret.txt")),
            Err(ResourceRuntimeError::OutsideWorkspace { .. })
        ));
        assert!(matches!(
            inspect_resource(directory.path(), Path::new("link.txt")),
            Err(ResourceRuntimeError::OutsideWorkspace { .. })
        ));
    }
}
