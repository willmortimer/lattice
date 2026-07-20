//! Governed local context API shared by HTTP and MCP.
//!
//! Read-only surface: search, bounded read, related, and build_context.
//! Mutations stay on the semantic command path — this module is not a second
//! write authority.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lattice_handlers::{get_backlinks_with_session, read_page, search_workspace_with_session};
use lattice_index::{parse_page, ExportPolicy, HybridSearchHit, Sensitivity};
use lattice_runtime::{hybrid_search_with_session_semantic, LatticeRuntime, WorkspaceSession};
use serde::{Deserialize, Serialize};

/// Hard cap on bytes returned from a single `/v1/read` call.
pub const MAX_READ_BYTES: usize = 256 * 1024;
/// Hard cap on assembled context bytes from `/v1/build_context`.
pub const MAX_CONTEXT_BYTES: usize = 64 * 1024;
/// Maximum search / related hit count accepted from clients.
pub const MAX_HIT_LIMIT: usize = 50;
const DEFAULT_HIT_LIMIT: usize = 10;

/// Errors returned by the local context API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Forbidden(String),
    Internal(String),
}

impl ApiError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::NotFound(_) => 404,
            Self::Forbidden(_) => 403,
            Self::Internal(_) => 500,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::NotFound(_) => "not_found",
            Self::Forbidden(_) => "forbidden",
            Self::Internal(_) => "internal",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(m)
            | Self::NotFound(m)
            | Self::Forbidden(m)
            | Self::Internal(m) => m,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_HIT_LIMIT).clamp(1, MAX_HIT_LIMIT)
}

/// Resolve an open session by workspace id, or open a read session at `root`.
pub fn resolve_session(
    runtime: &LatticeRuntime,
    workspace_id: Option<&str>,
    root: Option<&str>,
) -> Result<Arc<WorkspaceSession>, ApiError> {
    if let Some(id) = workspace_id.filter(|s| !s.is_empty()) {
        return runtime
            .get_session_by_id(id)
            .ok_or_else(|| ApiError::NotFound(format!("workspace session not found for id {id}")));
    }
    let root = root
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest("workspaceId or root is required".into()))?;
    runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(|err| ApiError::BadRequest(err.to_string()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
    /// `hybrid` (default) or `fts`.
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub path: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub score: f64,
    pub chunk_id: Option<String>,
    pub heading_path: Vec<String>,
    pub source_start_byte: Option<u64>,
    pub source_end_byte: Option<u64>,
    pub sensitivity: String,
    pub export_policy: String,
    pub provenance: Option<ProvenanceDto>,
    /// True when excerpt was withheld because of export_policy or private sensitivity.
    pub export_redacted: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceDto {
    pub content_hash: String,
    pub chunker_version: String,
    pub namespace_key: Option<String>,
    pub model_id: Option<String>,
    pub model_revision: Option<String>,
    pub instruction_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub workspace_id: String,
    pub mode: String,
    pub hits: Vec<SearchHitDto>,
}

pub fn api_search(runtime: &LatticeRuntime, params: SearchParams) -> Result<SearchResponse, ApiError> {
    if params.query.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".into()));
    }
    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let limit = clamp_limit(params.limit);
    let mode = params
        .mode
        .as_deref()
        .unwrap_or("hybrid")
        .trim()
        .to_ascii_lowercase();

    let hits = match mode.as_str() {
        "fts" => {
            let raw = search_workspace_with_session(&session, &params.query, limit)
                .map_err(ApiError::Internal)?;
            let mut hits = Vec::new();
            for hit in raw {
                let path = path_string(&hit.path);
                let (sensitivity, export_policy) =
                    policy_for_session_path(&session, &path).map_err(ApiError::Internal)?;
                if sensitivity == Sensitivity::Secret {
                    continue;
                }
                let (excerpt, redacted) =
                    redact_excerpt_for_export(hit.snippet.as_deref().unwrap_or(""), sensitivity, export_policy);
                hits.push(SearchHitDto {
                    path,
                    title: hit.title,
                    excerpt,
                    score: hit.rank,
                    chunk_id: None,
                    heading_path: Vec::new(),
                    source_start_byte: None,
                    source_end_byte: None,
                    sensitivity: sensitivity.as_str().to_string(),
                    export_policy: export_policy.as_str().to_string(),
                    provenance: None,
                    export_redacted: redacted,
                });
            }
            hits
        }
        "hybrid" => {
            let raw = hybrid_search_with_session_semantic(&session, &params.query, limit)
                .map_err(|err| ApiError::Internal(err.to_string()))?;
            raw.into_iter().map(hybrid_hit_to_dto).collect()
        }
        other => {
            return Err(ApiError::BadRequest(format!(
                "unsupported search mode '{other}' (use hybrid or fts)"
            )));
        }
    };

    Ok(SearchResponse {
        workspace_id: session.workspace_id().to_string(),
        mode,
        hits,
    })
}

fn hybrid_hit_to_dto(hit: HybridSearchHit) -> SearchHitDto {
    let (excerpt, redacted) = redact_excerpt_for_export(&hit.excerpt, hit.sensitivity, hit.export_policy);
    SearchHitDto {
        path: resource_path_from_uri(&hit.resource_uri),
        title: hit.title,
        excerpt,
        score: f64::from(hit.fused_score),
        chunk_id: Some(hit.chunk_id),
        heading_path: hit.heading_path,
        source_start_byte: Some(hit.source_start_byte),
        source_end_byte: Some(hit.source_end_byte),
        sensitivity: hit.sensitivity.as_str().to_string(),
        export_policy: hit.export_policy.as_str().to_string(),
        provenance: Some(ProvenanceDto {
            content_hash: hit.provenance.content_hash,
            chunker_version: hit.provenance.chunker_version,
            namespace_key: hit.provenance.namespace_key,
            model_id: hit.provenance.model_id,
            model_revision: hit.provenance.model_revision,
            instruction_version: hit.provenance.instruction_version,
        }),
        export_redacted: redacted,
    }
}

/// Secret hits are filtered before hydration. Private and ask/deny withhold
/// excerpts on the export API; allow returns content as-is.
fn redact_excerpt_for_export(
    excerpt: &str,
    sensitivity: Sensitivity,
    policy: ExportPolicy,
) -> (Option<String>, bool) {
    match sensitivity {
        Sensitivity::Secret => (None, true),
        Sensitivity::Private => (None, true),
        Sensitivity::Workspace => match policy {
            ExportPolicy::Allow => {
                if excerpt.is_empty() {
                    (None, false)
                } else {
                    (Some(excerpt.to_string()), false)
                }
            }
            ExportPolicy::Ask | ExportPolicy::Deny => (None, true),
        },
    }
}

fn policy_for_session_path(
    session: &WorkspaceSession,
    path: &str,
) -> Result<(Sensitivity, ExportPolicy), String> {
    session
        .ensure_index_warm()
        .map_err(|err| err.to_string())?;
    session
        .index()
        .export_policy_for_path(Path::new(path))
        .map_err(|err| err.to_string())
}

fn policy_from_page_content(path: &str, content: &str) -> (Sensitivity, ExportPolicy) {
    let page = parse_page(Path::new(path), content);
    (
        page.sensitivity
            .as_deref()
            .map(Sensitivity::parse)
            .unwrap_or(Sensitivity::Workspace),
        page.export_policy
            .as_deref()
            .map(ExportPolicy::parse)
            .unwrap_or(ExportPolicy::Ask),
    )
}

fn resource_path_from_uri(uri: &str) -> String {
    uri.strip_prefix("lattice://resource/")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri)
        .to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub path: String,
    #[serde(default)]
    pub start_byte: Option<u64>,
    #[serde(default)]
    pub end_byte: Option<u64>,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadResponse {
    pub workspace_id: String,
    pub path: String,
    pub revision: String,
    pub content: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub truncated: bool,
    pub total_bytes: u64,
    pub sensitivity: String,
    pub export_policy: String,
    /// True when content was withheld because of sensitivity or export_policy.
    pub export_redacted: bool,
}

pub fn api_read(runtime: &LatticeRuntime, params: ReadParams) -> Result<ReadResponse, ApiError> {
    if params.path.trim().is_empty() {
        return Err(ApiError::BadRequest("path must not be empty".into()));
    }
    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let root = session.root().to_string_lossy().into_owned();
    let page = read_page(root, params.path.clone()).map_err(|err| {
        if err.contains("not found") || err.contains("No such file") {
            ApiError::NotFound(err)
        } else {
            ApiError::BadRequest(err)
        }
    })?;

    let (sensitivity, export_policy) = match policy_for_session_path(&session, &params.path) {
        Ok(policy) => policy,
        Err(_) => policy_from_page_content(&params.path, &page.content),
    };
    // Prefer on-disk frontmatter when present (authoritative for export).
    let from_content = policy_from_page_content(&params.path, &page.content);
    let sensitivity = if page.content.starts_with("---") {
        from_content.0
    } else {
        sensitivity
    };
    let export_policy = if page.content.starts_with("---") {
        from_content.1
    } else {
        export_policy
    };

    if sensitivity == Sensitivity::Secret {
        return Err(ApiError::Forbidden(
            "resource is secret and cannot be exported via the context API".into(),
        ));
    }
    if sensitivity == Sensitivity::Private
        || matches!(export_policy, ExportPolicy::Deny | ExportPolicy::Ask)
    {
        return Ok(ReadResponse {
            workspace_id: session.workspace_id().to_string(),
            path: params.path,
            revision: page.revision,
            content: String::new(),
            start_byte: 0,
            end_byte: 0,
            truncated: false,
            total_bytes: page.content.len() as u64,
            sensitivity: sensitivity.as_str().to_string(),
            export_policy: export_policy.as_str().to_string(),
            export_redacted: true,
        });
    }

    let bytes = page.content.as_bytes();
    let total = bytes.len() as u64;
    let start = params.start_byte.unwrap_or(0).min(total);
    let end_cap = params.end_byte.unwrap_or(total).min(total);
    if end_cap < start {
        return Err(ApiError::BadRequest(
            "endByte must be greater than or equal to startByte".into(),
        ));
    }
    let max_bytes = params
        .max_bytes
        .unwrap_or(MAX_READ_BYTES)
        .clamp(1, MAX_READ_BYTES);
    let available = (end_cap - start) as usize;
    let take = available.min(max_bytes);
    let end = start + take as u64;
    let slice = &bytes[start as usize..end as usize];
    let content = String::from_utf8_lossy(slice).into_owned();
    let truncated = end < end_cap || end < total && params.end_byte.is_none() && take == max_bytes;

    Ok(ReadResponse {
        workspace_id: session.workspace_id().to_string(),
        path: params.path,
        revision: page.revision,
        content,
        start_byte: start,
        end_byte: end,
        truncated,
        total_bytes: total,
        sensitivity: sensitivity.as_str().to_string(),
        export_policy: export_policy.as_str().to_string(),
        export_redacted: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub path: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelatedHitDto {
    pub path: String,
    pub kind: String,
    pub score: f64,
    pub label: Option<String>,
    pub title: Option<String>,
    pub excerpt: Option<String>,
    pub sensitivity: String,
    pub export_policy: String,
    pub export_redacted: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelatedResponse {
    pub workspace_id: String,
    pub path: String,
    pub hits: Vec<RelatedHitDto>,
}

pub fn api_related(
    runtime: &LatticeRuntime,
    params: RelatedParams,
) -> Result<RelatedResponse, ApiError> {
    if params.path.trim().is_empty() {
        return Err(ApiError::BadRequest("path must not be empty".into()));
    }
    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let limit = clamp_limit(params.limit);

    let mut hits: Vec<RelatedHitDto> = Vec::new();

    let backlinks = get_backlinks_with_session(&session, &params.path).map_err(ApiError::Internal)?;
    for link in backlinks.into_iter().take(limit) {
        let path = path_string(&link.source_path);
        let (sensitivity, export_policy) =
            policy_for_session_path(&session, &path).unwrap_or((
                Sensitivity::Workspace,
                ExportPolicy::Ask,
            ));
        if sensitivity == Sensitivity::Secret {
            continue;
        }
        hits.push(RelatedHitDto {
            path,
            kind: format!("backlink-{}", link.kind.as_str()),
            score: 1.0,
            label: link.label,
            title: None,
            excerpt: None,
            sensitivity: sensitivity.as_str().to_string(),
            export_policy: export_policy.as_str().to_string(),
            export_redacted: false,
        });
    }

    if hits.len() < limit {
        let stem = Path::new(&params.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(params.path.as_str());
        if !stem.is_empty() {
            let remaining = limit - hits.len();
            let fts = search_workspace_with_session(&session, stem, remaining + hits.len())
                .map_err(ApiError::Internal)?;
            for hit in fts {
                let path = path_string(&hit.path);
                if path == params.path || hits.iter().any(|h| h.path == path) {
                    continue;
                }
                let (sensitivity, export_policy) =
                    policy_for_session_path(&session, &path).map_err(ApiError::Internal)?;
                if sensitivity == Sensitivity::Secret {
                    continue;
                }
                let (excerpt, redacted) = redact_excerpt_for_export(
                    hit.snippet.as_deref().unwrap_or(""),
                    sensitivity,
                    export_policy,
                );
                hits.push(RelatedHitDto {
                    path,
                    kind: "fts".into(),
                    score: hit.rank,
                    label: None,
                    title: Some(hit.title),
                    excerpt,
                    sensitivity: sensitivity.as_str().to_string(),
                    export_policy: export_policy.as_str().to_string(),
                    export_redacted: redacted,
                });
                if hits.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(RelatedResponse {
        workspace_id: session.workspace_id().to_string(),
        path: params.path,
        hits,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildContextParams {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextExcerpt {
    pub path: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub export_policy: String,
    pub export_redacted: bool,
    pub needs_consent: bool,
    pub provenance: Option<ProvenanceDto>,
    pub source_start_byte: Option<u64>,
    pub source_end_byte: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildContextResponse {
    pub workspace_id: String,
    pub query: String,
    pub excerpts: Vec<ContextExcerpt>,
    pub total_bytes: usize,
    pub truncated: bool,
    pub omitted_ask_or_deny: usize,
}

pub fn api_build_context(
    runtime: &LatticeRuntime,
    params: BuildContextParams,
) -> Result<BuildContextResponse, ApiError> {
    if params.query.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".into()));
    }
    let session = resolve_session(
        runtime,
        params.workspace_id.as_deref(),
        params.root.as_deref(),
    )?;
    let limit = clamp_limit(params.limit);
    let max_bytes = params
        .max_bytes
        .unwrap_or(MAX_CONTEXT_BYTES)
        .clamp(1, MAX_CONTEXT_BYTES);

    let hits = hybrid_search_with_session_semantic(&session, &params.query, limit)
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    let mut excerpts = Vec::new();
    let mut total_bytes = 0usize;
    let mut truncated = false;
    let mut omitted = 0usize;

    for hit in hits {
        // Secret never reaches here (filtered at hydration). Private is local-only:
        // treat like ask for export context assembly.
        if hit.sensitivity == Sensitivity::Private {
            omitted += 1;
            excerpts.push(ContextExcerpt {
                path: resource_path_from_uri(&hit.resource_uri),
                title: hit.title,
                excerpt: None,
                export_policy: hit.export_policy.as_str().to_string(),
                export_redacted: true,
                needs_consent: true,
                provenance: Some(ProvenanceDto {
                    content_hash: hit.provenance.content_hash,
                    chunker_version: hit.provenance.chunker_version,
                    namespace_key: hit.provenance.namespace_key,
                    model_id: hit.provenance.model_id,
                    model_revision: hit.provenance.model_revision,
                    instruction_version: hit.provenance.instruction_version,
                }),
                source_start_byte: Some(hit.source_start_byte),
                source_end_byte: Some(hit.source_end_byte),
            });
            continue;
        }
        match hit.export_policy {
            ExportPolicy::Deny => {
                omitted += 1;
                continue;
            }
            ExportPolicy::Ask => {
                // Do not exfiltrate freely: include a consent-flagged stub without text.
                omitted += 1;
                excerpts.push(ContextExcerpt {
                    path: resource_path_from_uri(&hit.resource_uri),
                    title: hit.title,
                    excerpt: None,
                    export_policy: ExportPolicy::Ask.as_str().to_string(),
                    export_redacted: true,
                    needs_consent: true,
                    provenance: Some(ProvenanceDto {
                        content_hash: hit.provenance.content_hash,
                        chunker_version: hit.provenance.chunker_version,
                        namespace_key: hit.provenance.namespace_key,
                        model_id: hit.provenance.model_id,
                        model_revision: hit.provenance.model_revision,
                        instruction_version: hit.provenance.instruction_version,
                    }),
                    source_start_byte: Some(hit.source_start_byte),
                    source_end_byte: Some(hit.source_end_byte),
                });
            }
            ExportPolicy::Allow => {
                let mut text = hit.excerpt;
                let remaining = max_bytes.saturating_sub(total_bytes);
                if remaining == 0 {
                    truncated = true;
                    break;
                }
                if text.len() > remaining {
                    // Truncate on a char boundary.
                    let mut end = remaining;
                    while end > 0 && !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    text.truncate(end);
                    truncated = true;
                }
                total_bytes += text.len();
                excerpts.push(ContextExcerpt {
                    path: resource_path_from_uri(&hit.resource_uri),
                    title: hit.title,
                    excerpt: Some(text),
                    export_policy: ExportPolicy::Allow.as_str().to_string(),
                    export_redacted: false,
                    needs_consent: false,
                    provenance: Some(ProvenanceDto {
                        content_hash: hit.provenance.content_hash,
                        chunker_version: hit.provenance.chunker_version,
                        namespace_key: hit.provenance.namespace_key,
                        model_id: hit.provenance.model_id,
                        model_revision: hit.provenance.model_revision,
                        instruction_version: hit.provenance.instruction_version,
                    }),
                    source_start_byte: Some(hit.source_start_byte),
                    source_end_byte: Some(hit.source_end_byte),
                });
                if truncated {
                    break;
                }
            }
        }
    }

    Ok(BuildContextResponse {
        workspace_id: session.workspace_id().to_string(),
        query: params.query,
        excerpts,
        total_bytes,
        truncated,
        omitted_ask_or_deny: omitted,
    })
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

trait BacklinkKindStr {
    fn as_str(&self) -> &'static str;
}

impl BacklinkKindStr for lattice_index::BacklinkKind {
    fn as_str(&self) -> &'static str {
        match self {
            lattice_index::BacklinkKind::Wiki => "wiki",
            lattice_index::BacklinkKind::Md => "md",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, LatticeRuntime) {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "API Fixture").unwrap();
        std::fs::write(
            dir.path().join("Notes.md"),
            "---\nexport_policy: allow\n---\n\n# Notes\n\nSearchable unique-phrase-xyz for context.\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Related.md"),
            "---\nexport_policy: allow\n---\n\n# Related\n\nSee [[Notes]] for details.\n",
        )
        .unwrap();
        (dir, LatticeRuntime::new())
    }

    #[test]
    fn search_and_read_round_trip() {
        let (dir, runtime) = fixture();
        let root = dir.path().to_string_lossy().into_owned();

        let search = api_search(
            &runtime,
            SearchParams {
                workspace_id: None,
                root: Some(root.clone()),
                query: "unique-phrase-xyz".into(),
                limit: Some(5),
                mode: Some("hybrid".into()),
            },
        )
        .unwrap();
        assert!(!search.hits.is_empty());
        assert_eq!(search.mode, "hybrid");
        assert!(search.hits.iter().any(|h| !h.export_redacted));

        let read = api_read(
            &runtime,
            ReadParams {
                workspace_id: Some(search.workspace_id.clone()),
                root: None,
                path: "Notes.md".into(),
                start_byte: Some(0),
                end_byte: Some(80),
                max_bytes: None,
            },
        )
        .unwrap();
        assert!(read.content.contains("Notes"));
        assert!(!read.export_redacted);
        assert_eq!(read.workspace_id, search.workspace_id);
    }

    #[test]
    fn fts_and_read_respect_export_policy() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Policy Fixture").unwrap();
        std::fs::write(
            dir.path().join("Secret.md"),
            "---\nsensitivity: secret\n---\n\n# Secret\n\nclassified-token-abc\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Ask.md"),
            "---\nexport_policy: ask\n---\n\n# Ask\n\nask-token-xyz\n",
        )
        .unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();

        let fts = api_search(
            &runtime,
            SearchParams {
                workspace_id: None,
                root: Some(root.clone()),
                query: "classified-token-abc".into(),
                limit: Some(5),
                mode: Some("fts".into()),
            },
        )
        .unwrap();
        assert!(
            fts.hits.iter().all(|h| h.path != "Secret.md"),
            "secret resources must not appear in FTS export results"
        );

        let ask = api_search(
            &runtime,
            SearchParams {
                workspace_id: None,
                root: Some(root.clone()),
                query: "ask-token-xyz".into(),
                limit: Some(5),
                mode: Some("fts".into()),
            },
        )
        .unwrap();
        assert!(ask.hits.iter().any(|h| h.path == "Ask.md" && h.export_redacted));

        let session = runtime.open_workspace_session(dir.path()).unwrap();
        let read = api_read(
            &runtime,
            ReadParams {
                workspace_id: Some(session.workspace_id().to_string()),
                root: None,
                path: "Ask.md".into(),
                start_byte: None,
                end_byte: None,
                max_bytes: None,
            },
        )
        .unwrap();
        assert!(read.export_redacted);
        assert!(read.content.is_empty());

        let secret = api_read(
            &runtime,
            ReadParams {
                workspace_id: Some(session.workspace_id().to_string()),
                root: None,
                path: "Secret.md".into(),
                start_byte: None,
                end_byte: None,
                max_bytes: None,
            },
        );
        assert!(matches!(secret, Err(ApiError::Forbidden(_))));
    }

    #[test]
    fn related_includes_backlinks() {
        let (dir, runtime) = fixture();
        let root = dir.path().to_string_lossy().into_owned();
        let related = api_related(
            &runtime,
            RelatedParams {
                workspace_id: None,
                root: Some(root),
                path: "Notes.md".into(),
                limit: Some(10),
            },
        )
        .unwrap();
        assert!(
            related
                .hits
                .iter()
                .any(|h| h.path.contains("Related") && h.kind.starts_with("backlink")),
            "expected backlink from Related.md: {:?}",
            related.hits
        );
    }

    #[test]
    fn build_context_flags_ask_policy() {
        let (dir, runtime) = fixture();
        let root = dir.path().to_string_lossy().into_owned();
        let ctx = api_build_context(
            &runtime,
            BuildContextParams {
                workspace_id: None,
                root: Some(root),
                query: "unique-phrase-xyz".into(),
                limit: Some(5),
                max_bytes: None,
            },
        )
        .unwrap();
        assert!(!ctx.excerpts.is_empty() || ctx.omitted_ask_or_deny > 0);
        for excerpt in &ctx.excerpts {
            if excerpt.export_policy == "ask" {
                assert!(excerpt.needs_consent);
                assert!(excerpt.excerpt.is_none());
            }
        }
    }

    #[test]
    fn missing_workspace_is_not_found() {
        let runtime = LatticeRuntime::new();
        let err = api_search(
            &runtime,
            SearchParams {
                workspace_id: Some("missing".into()),
                root: None,
                query: "x".into(),
                limit: None,
                mode: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.status_code(), 404);
    }
}
