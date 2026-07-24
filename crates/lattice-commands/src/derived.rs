//! Parse, status, and rebuild for `*.derived.yaml` resources (ADR 0022).
//!
//! Manifests declare inputs, a builder task, and an output path. Lineage and
//! lifecycle state live under `.lattice/derived/` so they stay rebuildable
//! operational state, not canonical content.
//!
//! Rebuilds write under `.lattice/derived/staging/<build-id>/` (via
//! `LATTICE_DERIVED_OUTPUT`), verify the staged file, then atomically promote
//! to the declared output. Failed or interrupted builds never overwrite
//! last-known-good output.

use lattice_core::OPERATIONAL_DIR;
use lattice_storage::{atomic_write_file, sha256_reader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::proposal::proposal_now_iso;
use crate::task::{TaskError, TaskRunner};
use crate::workflow::{path_matches_glob, resolve_workspace_path};

pub const DERIVED_FORMAT: &str = "lattice-derived-resource";
pub const SUPPORTED_VERSION: u32 = 1;
pub const DERIVED_DIR: &str = "derived";
pub const STAGING_DIR: &str = "staging";

/// Env var: absolute path where the builder must write its output artifact.
pub const ENV_DERIVED_OUTPUT: &str = "LATTICE_DERIVED_OUTPUT";
/// Env var: absolute staging directory for this build.
pub const ENV_DERIVED_STAGING: &str = "LATTICE_DERIVED_STAGING";
/// Env var: workspace-relative final output path (informational).
pub const ENV_DERIVED_OUTPUT_REL: &str = "LATTICE_DERIVED_OUTPUT_REL";

/// Errors from loading or rebuilding a derived resource.
#[derive(Debug, thiserror::Error)]
pub enum DerivedError {
    /// YAML failed structural validation after parse.
    #[error("invalid derived resource at {path}: {message}")]
    Invalid { path: PathBuf, message: String },

    /// YAML parse failure.
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// I/O while reading or writing derived artifacts.
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Nested task failure.
    #[error(transparent)]
    Task(#[from] TaskError),
}

pub type DerivedResult<T> = std::result::Result<T, DerivedError>;

/// Lifecycle state for a derived resource (docs/18, ADR 0022).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivedState {
    Current,
    Stale,
    Building,
    Failed,
}

impl DerivedState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Building => "building",
            Self::Failed => "failed",
        }
    }
}

/// Structured reasons a derived resource is not Current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivedStaleReason {
    NeverBuilt,
    InputChanged,
    InputMissing,
    OutputMissing,
    OutputChanged,
    BuilderFailed,
    BuilderChanged,
}

impl DerivedStaleReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeverBuilt => "never-built",
            Self::InputChanged => "input-changed",
            Self::InputMissing => "input-missing",
            Self::OutputMissing => "output-missing",
            Self::OutputChanged => "output-changed",
            Self::BuilderFailed => "builder-failed",
            Self::BuilderChanged => "builder-changed",
        }
    }
}

fn default_refresh_mode() -> String {
    "on-demand".into()
}

/// Builder block: currently a path to a `.task/` package or `task.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedBuilder {
    pub task: String,
}

/// Optional refresh policy (v1: recorded only; rebuild is on-demand).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedRefresh {
    #[serde(default = "default_refresh_mode")]
    pub mode: String,
}

impl Default for DerivedRefresh {
    fn default() -> Self {
        Self {
            mode: default_refresh_mode(),
        }
    }
}

/// Parsed `*.derived.yaml` document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedManifest {
    pub format: String,
    pub version: u32,
    pub output: String,
    pub inputs: Vec<String>,
    pub builder: DerivedBuilder,
    #[serde(default)]
    pub refresh: DerivedRefresh,
}

impl DerivedManifest {
    /// Load and validate a derived-resource manifest at `path`.
    pub fn load(path: &Path) -> DerivedResult<Self> {
        let text = fs::read_to_string(path).map_err(|source| DerivedError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse_str(&text, path)
    }

    /// Parse YAML text and validate as if loaded from `path`.
    pub fn parse_str(text: &str, path: &Path) -> DerivedResult<Self> {
        let manifest: DerivedManifest =
            serde_yaml::from_str(text).map_err(|source| DerivedError::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    fn check(&self, path: &Path) -> DerivedResult<()> {
        let invalid = |message: String| DerivedError::Invalid {
            path: path.to_path_buf(),
            message,
        };
        if self.format != DERIVED_FORMAT {
            return Err(invalid(format!(
                "expected format {DERIVED_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version == 0 || self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is not supported (expected 1..={SUPPORTED_VERSION})",
                self.version
            )));
        }
        if self.output.trim().is_empty() {
            return Err(invalid("output must be a non-empty path".into()));
        }
        if self.builder.task.trim().is_empty() {
            return Err(invalid("builder.task must be a non-empty path".into()));
        }
        if self.inputs.is_empty() {
            return Err(invalid("inputs must list at least one path or glob".into()));
        }
        for input in &self.inputs {
            if input.trim().is_empty() {
                return Err(invalid("inputs must not contain empty entries".into()));
            }
        }
        Ok(())
    }
}

/// One hashed input path recorded in lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedInputHash {
    /// Workspace-relative path to the input file.
    pub path: String,
    /// Content hash (`sha256:<hex>`), or `None` when the file is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Glob pattern from the manifest that produced this path, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Persisted lineage + lifecycle under `.lattice/derived/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedLineage {
    pub resource_path: String,
    pub state: DerivedState,
    pub builder_task: String,
    pub output: String,
    pub inputs: Vec<DerivedInputHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale_reasons: Vec<DerivedStaleReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_built_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Active staging build id while `state == Building`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_build_id: Option<String>,
}

/// Status DTO returned to CLI / desktop (live staleness recomputed on load).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedStatus {
    pub resource_path: String,
    pub state: DerivedState,
    pub output: String,
    pub builder_task: String,
    pub refresh_mode: String,
    pub inputs: Vec<DerivedInputHash>,
    pub current_inputs: Vec<DerivedInputHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale_reasons: Vec<DerivedStaleReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_built_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Directory holding derived lineage: `<workspace>/.lattice/derived/`.
pub fn derived_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(OPERATIONAL_DIR).join(DERIVED_DIR)
}

/// Staging root: `<workspace>/.lattice/derived/staging/`.
pub fn staging_root(workspace_root: &Path) -> PathBuf {
    derived_dir(workspace_root).join(STAGING_DIR)
}

fn normalize_rel(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn lineage_filename(resource_rel: &str) -> String {
    let normalized = normalize_rel(resource_rel);
    let safe: String = normalized
        .chars()
        .map(|c| match c {
            '/' | '\\' => '-',
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') => c,
            _ => '_',
        })
        .collect();
    format!("{safe}.json")
}

/// Absolute path to the lineage JSON for a workspace-relative derived resource.
pub fn lineage_path(workspace_root: &Path, resource_rel: &str) -> PathBuf {
    derived_dir(workspace_root).join(lineage_filename(resource_rel))
}

fn now_iso() -> String {
    proposal_now_iso()
}

fn lock_key(workspace_root: &Path, resource_rel: &str) -> String {
    format!(
        "{}::{}",
        workspace_root.display(),
        normalize_rel(resource_rel)
    )
}

fn resource_build_lock(workspace_root: &Path, resource_rel: &str) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = lock_key(workspace_root, resource_rel);
    let mut guard = map.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Load persisted lineage if present.
pub fn load_lineage(
    workspace_root: &Path,
    resource_rel: &str,
) -> DerivedResult<Option<DerivedLineage>> {
    let path = lineage_path(workspace_root, resource_rel);
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|source| DerivedError::Io {
        path: path.clone(),
        source,
    })?;
    let lineage: DerivedLineage =
        serde_json::from_str(&text).map_err(|err| DerivedError::Invalid {
            path,
            message: format!("failed to parse lineage: {err}"),
        })?;
    Ok(Some(lineage))
}

/// Persist lineage JSON under `.lattice/derived/`.
pub fn save_lineage(workspace_root: &Path, lineage: &DerivedLineage) -> DerivedResult<()> {
    let dir = derived_dir(workspace_root);
    fs::create_dir_all(&dir).map_err(|source| DerivedError::Io {
        path: dir.clone(),
        source,
    })?;
    let path = lineage_path(workspace_root, &lineage.resource_path);
    let payload = serde_json::to_string_pretty(lineage).map_err(|err| DerivedError::Invalid {
        path: path.clone(),
        message: format!("failed to serialize lineage: {err}"),
    })?;
    let mut file = File::create(&path).map_err(|source| DerivedError::Io {
        path: path.clone(),
        source,
    })?;
    file.write_all(payload.as_bytes())
        .map_err(|source| DerivedError::Io { path, source })?;
    Ok(())
}

fn is_glob_pattern(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

fn hash_file(path: &Path) -> DerivedResult<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let file = File::open(path).map_err(|source| DerivedError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let hash = sha256_reader(file).map_err(|source| DerivedError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Some(hash))
}

fn workspace_rel(workspace_root: &Path, absolute: &Path) -> String {
    let raw = absolute
        .strip_prefix(workspace_root)
        .unwrap_or(absolute)
        .to_string_lossy()
        .replace('\\', "/");
    // Collapse `foo/./bar` segments left by joining relative `./` refs.
    let mut parts = Vec::new();
    for part in raw.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn collect_under(dir: &Path, out: &mut Vec<PathBuf>) -> DerivedResult<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|source| DerivedError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| DerivedError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| DerivedError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            // Skip operational / VCS noise inside glob expansion.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == OPERATIONAL_DIR
                || name == ".git"
                || name == "node_modules"
                || name == ".venv"
                || name == "__pycache__"
            {
                continue;
            }
            collect_under(&path, out)?;
        } else if file_type.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Resolve one manifest input (literal path or simple glob) to hashed files.
pub fn hash_input_pattern(
    workspace_root: &Path,
    derived_path: &Path,
    pattern: &str,
) -> DerivedResult<Vec<DerivedInputHash>> {
    let pattern = pattern.trim();
    let base = resolve_workspace_path(workspace_root, derived_path, pattern);

    if !is_glob_pattern(pattern) {
        let abs = if base.exists() {
            base
        } else {
            // Fall back to workspace-root join when the relative resolve missed.
            workspace_root.join(pattern.trim_start_matches("./"))
        };
        let rel = workspace_rel(workspace_root, &abs);
        let hash = hash_file(&abs)?;
        return Ok(vec![DerivedInputHash {
            path: rel,
            hash,
            pattern: None,
        }]);
    }

    // Glob: walk from the longest non-glob prefix under the derived parent / workspace.
    let search_root = glob_search_root(workspace_root, derived_path, pattern);
    let mut candidates = Vec::new();
    collect_under(&search_root, &mut candidates)?;

    let mut matched = Vec::new();
    for absolute in candidates {
        let rel = workspace_rel(workspace_root, &absolute);
        // Match against both workspace-relative and pattern-as-written forms.
        let pattern_norm = normalize_rel(pattern);
        if path_matches_glob(&rel, &pattern_norm)
            || path_matches_glob(
                &rel,
                &workspace_rel(
                    workspace_root,
                    &resolve_workspace_path(workspace_root, derived_path, pattern),
                ),
            )
            || matches_relative_glob(derived_path, workspace_root, &rel, pattern)
        {
            let hash = hash_file(&absolute)?;
            matched.push(DerivedInputHash {
                path: rel,
                hash,
                pattern: Some(pattern.to_string()),
            });
        }
    }
    matched.sort_by(|a, b| a.path.cmp(&b.path));
    if matched.is_empty() {
        // Preserve the pattern as a missing sentinel so status stays stale.
        matched.push(DerivedInputHash {
            path: normalize_rel(pattern),
            hash: None,
            pattern: Some(pattern.to_string()),
        });
    }
    Ok(matched)
}

fn matches_relative_glob(
    derived_path: &Path,
    workspace_root: &Path,
    workspace_rel_path: &str,
    pattern: &str,
) -> bool {
    let parent = derived_path
        .parent()
        .unwrap_or(workspace_root)
        .strip_prefix(workspace_root)
        .ok();
    let Some(parent) = parent else {
        return false;
    };
    let parent_rel = parent.to_string_lossy().replace('\\', "/");
    let joined = if parent_rel.is_empty() {
        normalize_rel(pattern)
    } else {
        format!(
            "{}/{}",
            parent_rel.trim_end_matches('/'),
            normalize_rel(pattern)
        )
    };
    path_matches_glob(workspace_rel_path, &joined)
}

fn glob_search_root(workspace_root: &Path, derived_path: &Path, pattern: &str) -> PathBuf {
    let trimmed = pattern.trim().trim_start_matches("./");
    let prefix_end = trimmed.find(['*', '?']).unwrap_or(trimmed.len());
    let prefix = &trimmed[..prefix_end];
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return derived_path
            .parent()
            .unwrap_or(workspace_root)
            .to_path_buf();
    }
    let candidate = resolve_workspace_path(workspace_root, derived_path, prefix);
    if candidate.is_dir() {
        candidate
    } else if let Some(parent) = candidate.parent() {
        if parent.exists() {
            parent.to_path_buf()
        } else {
            workspace_root.to_path_buf()
        }
    } else {
        workspace_root.to_path_buf()
    }
}

/// Hash all declared inputs for a derived resource.
pub fn hash_inputs(
    workspace_root: &Path,
    derived_path: &Path,
    manifest: &DerivedManifest,
) -> DerivedResult<Vec<DerivedInputHash>> {
    let mut all = Vec::new();
    for pattern in &manifest.inputs {
        all.extend(hash_input_pattern(workspace_root, derived_path, pattern)?);
    }
    all.sort_by(|a, b| a.path.cmp(&b.path));
    all.dedup_by(|a, b| a.path == b.path);
    Ok(all)
}

fn inputs_match(recorded: &[DerivedInputHash], current: &[DerivedInputHash]) -> bool {
    if recorded.len() != current.len() {
        return false;
    }
    recorded
        .iter()
        .zip(current.iter())
        .all(|(a, b)| a.path == b.path && a.hash.is_some() && a.hash == b.hash)
}

fn any_input_missing(current: &[DerivedInputHash]) -> bool {
    current.iter().any(|i| i.hash.is_none())
}

fn resolve_builder_task(workspace_root: &Path, derived_path: &Path, task_ref: &str) -> PathBuf {
    let resolved = resolve_workspace_path(workspace_root, derived_path, task_ref);
    if resolved.is_file()
        && resolved
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "task.yaml" || n == "task.yml")
    {
        return resolved.parent().unwrap_or(workspace_root).to_path_buf();
    }
    resolved
}

/// Resolve the declared output path relative to the derived manifest.
///
/// Unlike [`resolve_workspace_path`], this does not fall back to the workspace
/// root when the file is missing — outputs are created on first successful
/// promote and must keep a stable destination beside the manifest.
fn resolve_output_path(workspace_root: &Path, derived_path: &Path, output: &str) -> PathBuf {
    let candidate = Path::new(output);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    derived_path.parent().unwrap_or(workspace_root).join(output)
}

/// Content hash of the builder task package (sorted path+file hashes).
pub fn hash_builder_package(builder_path: &Path) -> DerivedResult<Option<String>> {
    if !builder_path.exists() {
        return Ok(None);
    }
    let mut files = Vec::new();
    if builder_path.is_file() {
        files.push(builder_path.to_path_buf());
    } else {
        collect_under(builder_path, &mut files)?;
    }
    files.sort();
    if files.is_empty() {
        return Ok(None);
    }

    let mut digest_input = Vec::new();
    for absolute in &files {
        let rel = absolute
            .strip_prefix(builder_path)
            .unwrap_or(absolute.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let Some(hash) = hash_file(absolute)? else {
            continue;
        };
        digest_input.extend_from_slice(rel.as_bytes());
        digest_input.push(0);
        digest_input.extend_from_slice(hash.as_bytes());
        digest_input.push(b'\n');
    }
    if digest_input.is_empty() {
        return Ok(None);
    }
    let hash =
        sha256_reader(std::io::Cursor::new(digest_input)).map_err(|source| DerivedError::Io {
            path: builder_path.to_path_buf(),
            source,
        })?;
    Ok(Some(hash))
}

fn promote_staged_output(staged: &Path, final_output: &Path) -> DerivedResult<()> {
    let bytes = {
        let mut file = File::open(staged).map_err(|source| DerivedError::Io {
            path: staged.to_path_buf(),
            source,
        })?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|source| DerivedError::Io {
                path: staged.to_path_buf(),
                source,
            })?;
        buf
    };
    if let Some(parent) = final_output.parent() {
        fs::create_dir_all(parent).map_err(|source| DerivedError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    atomic_write_file(final_output, &bytes).map_err(|err| DerivedError::Io {
        path: final_output.to_path_buf(),
        source: std::io::Error::other(err.to_string()),
    })?;
    Ok(())
}

/// Remove staging directories that are not the active build (if any).
pub fn cleanup_abandoned_staging(
    workspace_root: &Path,
    keep_build_id: Option<&str>,
) -> DerivedResult<()> {
    let root = staging_root(workspace_root);
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&root).map_err(|source| DerivedError::Io {
        path: root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| DerivedError::Io {
            path: root.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if keep_build_id.is_some_and(|keep| keep == name) {
            continue;
        }
        let _ = fs::remove_dir_all(&path);
    }
    Ok(())
}

fn compute_stale_reasons(
    lineage: Option<&DerivedLineage>,
    current_inputs: &[DerivedInputHash],
    current_builder_hash: Option<&str>,
    current_output_hash: Option<&str>,
    output_exists: bool,
) -> (DerivedState, Vec<DerivedStaleReason>) {
    let Some(record) = lineage else {
        return (DerivedState::Stale, vec![DerivedStaleReason::NeverBuilt]);
    };

    if record.state == DerivedState::Building {
        return (DerivedState::Building, record.stale_reasons.clone());
    }

    let mut reasons = Vec::new();

    if record.state == DerivedState::Failed {
        reasons.push(DerivedStaleReason::BuilderFailed);
    }

    if record.output_hash.is_none() && record.last_built_at.is_none() && record.inputs.is_empty() {
        reasons.push(DerivedStaleReason::NeverBuilt);
    }

    if any_input_missing(current_inputs) {
        reasons.push(DerivedStaleReason::InputMissing);
    } else if !inputs_match(&record.inputs, current_inputs) {
        reasons.push(DerivedStaleReason::InputChanged);
    }

    match (&record.builder_hash, current_builder_hash) {
        (Some(recorded), Some(current)) if recorded != current => {
            reasons.push(DerivedStaleReason::BuilderChanged);
        }
        (Some(_), None) => reasons.push(DerivedStaleReason::BuilderChanged),
        (None, _) if record.last_built_at.is_some() => {
            reasons.push(DerivedStaleReason::BuilderChanged);
        }
        _ => {}
    }

    if !output_exists {
        if record.output_hash.is_some() || record.last_built_at.is_some() {
            reasons.push(DerivedStaleReason::OutputMissing);
        }
    } else {
        match (&record.output_hash, current_output_hash) {
            (Some(recorded), Some(current)) if recorded != current => {
                reasons.push(DerivedStaleReason::OutputChanged);
            }
            (Some(_), None) => reasons.push(DerivedStaleReason::OutputMissing),
            (None, Some(_)) if record.last_built_at.is_some() => {
                // Legacy lineage without output_hash: treat as changed until rebuilt.
                reasons.push(DerivedStaleReason::OutputChanged);
            }
            _ => {}
        }
    }

    reasons.sort_by_key(|r| r.as_str());
    reasons.dedup();

    if record.state == DerivedState::Failed {
        return (DerivedState::Failed, reasons);
    }

    let verified_current = reasons.is_empty()
        && record.output_hash.is_some()
        && record.builder_hash.is_some()
        && !record.inputs.is_empty()
        && output_exists
        && current_output_hash.is_some()
        && current_builder_hash.is_some()
        && inputs_match(&record.inputs, current_inputs)
        && record.output_hash.as_deref() == current_output_hash
        && record.builder_hash.as_deref() == current_builder_hash;

    if verified_current {
        (DerivedState::Current, Vec::new())
    } else {
        if reasons.is_empty() {
            reasons.push(DerivedStaleReason::NeverBuilt);
        }
        (DerivedState::Stale, reasons)
    }
}

/// Compute live status: re-hash inputs/builder/output and compare against lineage.
pub fn load_derived_status(
    workspace_root: &Path,
    resource_rel: &str,
) -> DerivedResult<DerivedStatus> {
    let derived_path = workspace_root.join(resource_rel);
    let manifest = DerivedManifest::load(&derived_path)?;
    let current_inputs = hash_inputs(workspace_root, &derived_path, &manifest)?;
    let lineage = load_lineage(workspace_root, resource_rel)?;

    let builder_path = resolve_builder_task(workspace_root, &derived_path, &manifest.builder.task);
    let builder_rel = workspace_rel(workspace_root, &builder_path);
    let output_abs = resolve_output_path(workspace_root, &derived_path, &manifest.output);
    let output_rel = workspace_rel(workspace_root, &output_abs);

    let current_builder_hash = hash_builder_package(&builder_path)?;
    let current_output_hash = hash_file(&output_abs)?;
    let output_exists = output_abs.is_file();

    // Drop abandoned staging when not actively building.
    let keep = lineage
        .as_ref()
        .filter(|l| l.state == DerivedState::Building)
        .and_then(|l| l.active_build_id.as_deref());
    let _ = cleanup_abandoned_staging(workspace_root, keep);

    let (state, stale_reasons) = compute_stale_reasons(
        lineage.as_ref(),
        &current_inputs,
        current_builder_hash.as_deref(),
        current_output_hash.as_deref(),
        output_exists,
    );

    let (recorded_inputs, output_hash, builder_hash, last_built_at, last_error) = match &lineage {
        Some(record) => (
            record.inputs.clone(),
            record.output_hash.clone(),
            record.builder_hash.clone(),
            record.last_built_at.clone(),
            if state == DerivedState::Failed {
                record.last_error.clone()
            } else if state == DerivedState::Current {
                None
            } else {
                record.last_error.clone()
            },
        ),
        None => (Vec::new(), None, None, None, None),
    };

    Ok(DerivedStatus {
        resource_path: normalize_rel(resource_rel),
        state,
        output: output_rel,
        builder_task: builder_rel,
        refresh_mode: manifest.refresh.mode,
        inputs: recorded_inputs,
        current_inputs,
        output_hash,
        builder_hash,
        stale_reasons,
        last_built_at,
        last_error,
    })
}

/// Run the declared builder task into staging, verify, and atomically promote.
pub fn rebuild_derived(
    workspace_root: &Path,
    resource_rel: &str,
    runner: &TaskRunner,
) -> DerivedResult<DerivedStatus> {
    let lock = resource_build_lock(workspace_root, resource_rel);
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let derived_path = workspace_root.join(resource_rel);
    let manifest = DerivedManifest::load(&derived_path)?;
    let current_inputs = hash_inputs(workspace_root, &derived_path, &manifest)?;
    let builder_path = resolve_builder_task(workspace_root, &derived_path, &manifest.builder.task);
    let builder_rel = workspace_rel(workspace_root, &builder_path);
    let output_abs = resolve_output_path(workspace_root, &derived_path, &manifest.output);
    let output_rel = workspace_rel(workspace_root, &output_abs);
    let builder_hash = hash_builder_package(&builder_path)?;

    let prior = load_lineage(workspace_root, resource_rel)?.unwrap_or(DerivedLineage {
        resource_path: normalize_rel(resource_rel),
        state: DerivedState::Stale,
        builder_task: builder_rel.clone(),
        output: output_rel.clone(),
        inputs: Vec::new(),
        output_hash: None,
        builder_hash: None,
        stale_reasons: vec![DerivedStaleReason::NeverBuilt],
        last_built_at: None,
        last_error: None,
        active_build_id: None,
    });

    // Preserve last-known-good on disk; never overwrite until promote.
    let prior_output_hash = prior.output_hash.clone();
    let prior_last_built_at = prior.last_built_at.clone();

    let build_id = uuid::Uuid::now_v7().to_string();
    cleanup_abandoned_staging(workspace_root, None)?;

    let staging_dir = staging_root(workspace_root).join(&build_id);
    fs::create_dir_all(&staging_dir).map_err(|source| DerivedError::Io {
        path: staging_dir.clone(),
        source,
    })?;

    let output_name = output_abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into());
    let staged_output = staging_dir.join(&output_name);

    let building = DerivedLineage {
        resource_path: normalize_rel(resource_rel),
        state: DerivedState::Building,
        builder_task: builder_rel.clone(),
        output: output_rel.clone(),
        inputs: current_inputs.clone(),
        output_hash: prior_output_hash.clone(),
        builder_hash: prior.builder_hash.clone(),
        stale_reasons: Vec::new(),
        last_built_at: prior_last_built_at.clone(),
        last_error: None,
        active_build_id: Some(build_id.clone()),
    };
    save_lineage(workspace_root, &building)?;

    let staged_output_str = staged_output.to_string_lossy().into_owned();
    let staging_dir_str = staging_dir.to_string_lossy().into_owned();
    let extra_env = [
        (ENV_DERIVED_OUTPUT, staged_output_str.as_str()),
        (ENV_DERIVED_STAGING, staging_dir_str.as_str()),
        (ENV_DERIVED_OUTPUT_REL, output_rel.as_str()),
    ];

    let run_result = runner.run_with_env(&builder_path, &extra_env);

    let finish_failed = |message: String| -> DerivedResult<DerivedStatus> {
        let lineage = DerivedLineage {
            resource_path: normalize_rel(resource_rel),
            state: DerivedState::Failed,
            builder_task: builder_rel.clone(),
            output: output_rel.clone(),
            inputs: current_inputs.clone(),
            output_hash: prior_output_hash.clone(),
            builder_hash: prior.builder_hash.clone(),
            stale_reasons: vec![DerivedStaleReason::BuilderFailed],
            last_built_at: prior_last_built_at.clone(),
            last_error: Some(message),
            active_build_id: None,
        };
        save_lineage(workspace_root, &lineage)?;
        let _ = cleanup_abandoned_staging(workspace_root, None);
        load_derived_status(workspace_root, resource_rel)
    };

    match run_result {
        Ok(output) if output.exit_code == 0 => {
            if !staged_output.is_file() {
                return finish_failed(format!(
                    "builder exited 0 but did not write {ENV_DERIVED_OUTPUT} ({})",
                    staged_output.display()
                ));
            }
            let new_output_hash =
                hash_file(&staged_output)?.ok_or_else(|| DerivedError::Invalid {
                    path: staged_output.clone(),
                    message: "staged output vanished before hash".into(),
                })?;

            // Promote only after verification; final path untouched until here.
            if let Err(err) = promote_staged_output(&staged_output, &output_abs) {
                let _ = cleanup_abandoned_staging(workspace_root, None);
                return finish_failed(format!("failed to promote staged output: {err}"));
            }

            let inputs = hash_inputs(workspace_root, &derived_path, &manifest)?;
            let lineage = DerivedLineage {
                resource_path: normalize_rel(resource_rel),
                state: DerivedState::Current,
                builder_task: builder_rel,
                output: output_rel,
                inputs,
                output_hash: Some(new_output_hash),
                builder_hash,
                stale_reasons: Vec::new(),
                last_built_at: Some(now_iso()),
                last_error: None,
                active_build_id: None,
            };
            save_lineage(workspace_root, &lineage)?;
            let _ = cleanup_abandoned_staging(workspace_root, None);
            load_derived_status(workspace_root, resource_rel)
        }
        Ok(output) => {
            let message = if output.stderr.trim().is_empty() {
                format!("builder exited with code {}", output.exit_code)
            } else {
                output.stderr.trim().to_string()
            };
            finish_failed(message)
        }
        Err(err) => {
            let message = err.to_string();
            let _ = finish_failed(message);
            // Surface the task error after persisting failed state.
            Err(DerivedError::Task(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_env::EnvProvider;
    use std::os::unix::fs::PermissionsExt;
    use std::thread;
    use std::time::Duration;

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn fixture_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Reports/src")).unwrap();
        fs::create_dir_all(root.join("Reports/Build Summary.task")).unwrap();
        fs::write(root.join("Reports/input.txt"), "hello-input\n").unwrap();
        fs::write(root.join("Reports/src/a.txt"), "a\n").unwrap();
        fs::write(
            root.join("Reports/Summary.derived.yaml"),
            r#"format: lattice-derived-resource
version: 1
output: ./dist/index.html
inputs:
  - ./input.txt
  - ./src/**
builder:
  task: ./Build Summary.task/task.yaml
refresh:
  mode: on-demand
"#,
        )
        .unwrap();
        fs::write(
            root.join("Reports/Build Summary.task/task.yaml"),
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
  project: .
entrypoint:
  command: [python, main.py]
limits:
  timeout_seconds: 30
"#,
        )
        .unwrap();
        fs::write(
            root.join("Reports/Build Summary.task/pyproject.toml"),
            "[project]\nname = \"build-summary\"\nversion = \"0.0.0\"\nrequires-python = \">=3.11\"\n",
        )
        .unwrap();
        fs::write(
            root.join("Reports/Build Summary.task/main.py"),
            "print('unused')\n",
        )
        .unwrap();
        dir
    }

    fn staging_aware_uv(bin: &Path, python_path: &Path, body: &str) {
        let uv_script = format!(
            r#"#!/bin/sh
if [ "$1" = "python" ] && [ "$2" = "find" ]; then
  printf '%s\n' '{python}'
  exit 0
fi
{body}
"#,
            python = python_path.display(),
        );
        write_executable(&bin.join("uv"), &uv_script);
        write_executable(python_path, "#!/bin/sh\nexit 0\n");
    }

    fn success_builder_body() -> &'static str {
        r#"out="${LATTICE_DERIVED_OUTPUT:?missing LATTICE_DERIVED_OUTPUT}"
mkdir -p "$(dirname "$out")"
printf 'built\n' > "$out"
exit 0
"#
    }

    fn fail_builder_body() -> &'static str {
        r#"# Intentionally fail without writing staging output.
exit 1
"#
    }

    fn make_runner(root: &Path) -> (PathBuf, TaskRunner) {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let path = std::env::join_paths([bin.as_path(), Path::new("/bin"), Path::new("/usr/bin")])
            .unwrap();
        (bin, TaskRunner::with_env(EnvProvider::with_path(path)))
    }

    #[test]
    fn parses_valid_manifest() {
        let dir = fixture_workspace();
        let path = dir.path().join("Reports/Summary.derived.yaml");
        let m = DerivedManifest::load(&path).unwrap();
        assert_eq!(m.format, DERIVED_FORMAT);
        assert_eq!(m.output, "./dist/index.html");
        assert_eq!(m.inputs.len(), 2);
        assert_eq!(m.builder.task, "./Build Summary.task/task.yaml");
        assert_eq!(m.refresh.mode, "on-demand");
    }

    #[test]
    fn rejects_wrong_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.derived.yaml");
        fs::write(
            &path,
            "format: other\nversion: 1\noutput: ./out\ninputs: [./a]\nbuilder:\n  task: ./t.task\n",
        )
        .unwrap();
        let err = DerivedManifest::load(&path).unwrap_err();
        assert!(matches!(err, DerivedError::Invalid { .. }));
    }

    #[test]
    fn never_built_is_stale_and_input_change_stales_after_current() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";

        let status = load_derived_status(root, rel).unwrap();
        assert_eq!(status.state, DerivedState::Stale);
        assert!(status
            .stale_reasons
            .contains(&DerivedStaleReason::NeverBuilt));
        assert!(status.inputs.is_empty());
        assert!(status
            .current_inputs
            .iter()
            .any(|i| i.path.ends_with("input.txt") && i.hash.is_some()));
        assert!(status
            .current_inputs
            .iter()
            .any(|i| i.path.ends_with("src/a.txt") && i.hash.is_some()));

        let (bin, runner) = make_runner(root);
        let python_path = bin.join("python");
        staging_aware_uv(&bin, &python_path, success_builder_body());
        let after = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(after.state, DerivedState::Current);
        assert!(after.stale_reasons.is_empty());
        assert!(after.output_hash.is_some());
        assert!(after.builder_hash.is_some());
        assert!(
            root.join(&after.output).is_file(),
            "missing output at {}",
            after.output
        );

        fs::write(root.join("Reports/input.txt"), "changed\n").unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
        assert!(stale
            .stale_reasons
            .contains(&DerivedStaleReason::InputChanged));
    }

    #[test]
    fn rebuild_marks_current_with_fake_uv() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";
        let out = root.join("Reports/dist/index.html");

        let (bin, runner) = make_runner(root);
        let python_path = bin.join("python");
        staging_aware_uv(&bin, &python_path, success_builder_body());

        let before = load_derived_status(root, rel).unwrap();
        assert_eq!(before.state, DerivedState::Stale);

        let after = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(after.state, DerivedState::Current);
        assert!(after.last_built_at.is_some());
        assert_eq!(after.builder_task, "Reports/Build Summary.task");
        assert!(!after.inputs.is_empty());
        assert!(out.is_file(), "expected output file at {}", out.display());
        assert_eq!(after.output, "Reports/dist/index.html");
        assert!(after.output_hash.is_some());
        assert!(staging_root(root)
            .read_dir()
            .map(|mut d| d.next().is_none())
            .unwrap_or(true));

        fs::write(root.join("Reports/input.txt"), "again\n").unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
    }

    #[test]
    fn missing_output_is_stale() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";
        let out = root.join("Reports/dist/index.html");

        let (bin, runner) = make_runner(root);
        staging_aware_uv(&bin, &bin.join("python"), success_builder_body());
        let after = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(after.state, DerivedState::Current);

        fs::remove_file(&out).unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
        assert!(stale
            .stale_reasons
            .contains(&DerivedStaleReason::OutputMissing));
    }

    #[test]
    fn externally_modified_output_is_stale() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";
        let out = root.join("Reports/dist/index.html");

        let (bin, runner) = make_runner(root);
        staging_aware_uv(&bin, &bin.join("python"), success_builder_body());
        assert_eq!(
            rebuild_derived(root, rel, &runner).unwrap().state,
            DerivedState::Current
        );

        fs::write(&out, "tampered\n").unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
        assert!(stale
            .stale_reasons
            .contains(&DerivedStaleReason::OutputChanged));
    }

    #[test]
    fn builder_change_is_stale() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";

        let (bin, runner) = make_runner(root);
        staging_aware_uv(&bin, &bin.join("python"), success_builder_body());
        assert_eq!(
            rebuild_derived(root, rel, &runner).unwrap().state,
            DerivedState::Current
        );

        fs::write(
            root.join("Reports/Build Summary.task/main.py"),
            "print('changed-builder')\n",
        )
        .unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
        assert!(stale
            .stale_reasons
            .contains(&DerivedStaleReason::BuilderChanged));
    }

    #[test]
    fn failed_build_preserves_prior_output() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";
        let out = root.join("Reports/dist/index.html");

        let (bin, runner) = make_runner(root);
        staging_aware_uv(&bin, &bin.join("python"), success_builder_body());
        let ok = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(ok.state, DerivedState::Current);
        let prior_bytes = fs::read(&out).unwrap();
        let prior_hash = ok.output_hash.clone();

        staging_aware_uv(&bin, &bin.join("python"), fail_builder_body());
        let failed = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(failed.state, DerivedState::Failed);
        assert!(failed
            .stale_reasons
            .contains(&DerivedStaleReason::BuilderFailed));
        assert_eq!(fs::read(&out).unwrap(), prior_bytes);
        assert_eq!(failed.output_hash, prior_hash);
    }

    #[test]
    fn partial_staging_never_promotes() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";
        let out = root.join("Reports/dist/index.html");

        let (bin, runner) = make_runner(root);
        // Exit 0 without writing LATTICE_DERIVED_OUTPUT.
        staging_aware_uv(&bin, &bin.join("python"), "exit 0\n");
        let failed = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(failed.state, DerivedState::Failed);
        assert!(!out.exists());
        assert!(staging_root(root)
            .read_dir()
            .map(|mut d| d.next().is_none())
            .unwrap_or(true));
    }

    #[test]
    fn concurrent_builds_serialize() {
        let dir = fixture_workspace();
        let root = dir.path().to_path_buf();
        let rel = "Reports/Summary.derived.yaml";

        let (bin, runner) = make_runner(&root);
        let slow_body = r#"out="${LATTICE_DERIVED_OUTPUT:?}"
mkdir -p "$(dirname "$out")"
printf 'built-%s\n' "$$" > "$out"
sleep 0.15
exit 0
"#;
        staging_aware_uv(&bin, &bin.join("python"), slow_body);

        let root_a = root.clone();
        let root_b = root.clone();
        let runner_a = runner.clone();
        let runner_b = runner.clone();
        let handle_a = thread::spawn(move || rebuild_derived(&root_a, rel, &runner_a));
        thread::sleep(Duration::from_millis(20));
        let handle_b = thread::spawn(move || rebuild_derived(&root_b, rel, &runner_b));

        let a = handle_a.join().unwrap().unwrap();
        let b = handle_b.join().unwrap().unwrap();
        assert_eq!(a.state, DerivedState::Current);
        assert_eq!(b.state, DerivedState::Current);
        assert!(root.join("Reports/dist/index.html").is_file());
        let status = load_derived_status(&root, rel).unwrap();
        assert_eq!(status.state, DerivedState::Current);
    }
}
