//! Parse, status, and rebuild for `*.derived.yaml` resources (ADR 0022).
//!
//! Manifests declare inputs, a builder task, and an output path. Lineage and
//! lifecycle state live under `.lattice/derived/` so they stay rebuildable
//! operational state, not canonical content.

use lattice_core::OPERATIONAL_DIR;
use lattice_storage::sha256_reader;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::proposal::proposal_now_iso;
use crate::task::{TaskError, TaskRunner};
use crate::workflow::{path_matches_glob, resolve_workspace_path};

pub const DERIVED_FORMAT: &str = "lattice-derived-resource";
pub const SUPPORTED_VERSION: u32 = 1;
pub const DERIVED_DIR: &str = "derived";

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
    pub last_built_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
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
    pub last_built_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Directory holding derived lineage: `<workspace>/.lattice/derived/`.
pub fn derived_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(OPERATIONAL_DIR).join(DERIVED_DIR)
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
            if name == OPERATIONAL_DIR || name == ".git" || name == "node_modules" {
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

/// Compute live status: re-hash inputs and compare against recorded lineage.
pub fn load_derived_status(
    workspace_root: &Path,
    resource_rel: &str,
) -> DerivedResult<DerivedStatus> {
    let derived_path = workspace_root.join(resource_rel);
    let manifest = DerivedManifest::load(&derived_path)?;
    let current_inputs = hash_inputs(workspace_root, &derived_path, &manifest)?;
    let lineage = load_lineage(workspace_root, resource_rel)?;

    let builder_rel = workspace_rel(
        workspace_root,
        &resolve_builder_task(workspace_root, &derived_path, &manifest.builder.task),
    );
    let output_rel = workspace_rel(
        workspace_root,
        &resolve_workspace_path(workspace_root, &derived_path, &manifest.output),
    );

    let (state, recorded_inputs, last_built_at, last_error) = match lineage {
        Some(record) if record.state == DerivedState::Building => (
            DerivedState::Building,
            record.inputs,
            record.last_built_at,
            record.last_error,
        ),
        Some(record) if record.state == DerivedState::Failed => {
            // Failed stays failed until a successful rebuild, unless inputs
            // still match a prior success window (still show failed).
            (
                DerivedState::Failed,
                record.inputs,
                record.last_built_at,
                record.last_error,
            )
        }
        Some(record) if inputs_match(&record.inputs, &current_inputs) => (
            DerivedState::Current,
            record.inputs,
            record.last_built_at,
            None,
        ),
        Some(record) => (
            DerivedState::Stale,
            record.inputs,
            record.last_built_at,
            record.last_error,
        ),
        None => (DerivedState::Stale, Vec::new(), None, None),
    };

    Ok(DerivedStatus {
        resource_path: normalize_rel(resource_rel),
        state,
        output: output_rel,
        builder_task: builder_rel,
        refresh_mode: manifest.refresh.mode,
        inputs: recorded_inputs,
        current_inputs,
        last_built_at,
        last_error,
    })
}

/// Run the declared builder task and update lineage on success / failure.
pub fn rebuild_derived(
    workspace_root: &Path,
    resource_rel: &str,
    runner: &TaskRunner,
) -> DerivedResult<DerivedStatus> {
    let derived_path = workspace_root.join(resource_rel);
    let manifest = DerivedManifest::load(&derived_path)?;
    let current_inputs = hash_inputs(workspace_root, &derived_path, &manifest)?;
    let builder_path = resolve_builder_task(workspace_root, &derived_path, &manifest.builder.task);
    let builder_rel = workspace_rel(workspace_root, &builder_path);
    let output_rel = workspace_rel(
        workspace_root,
        &resolve_workspace_path(workspace_root, &derived_path, &manifest.output),
    );

    let mut building = DerivedLineage {
        resource_path: normalize_rel(resource_rel),
        state: DerivedState::Building,
        builder_task: builder_rel.clone(),
        output: output_rel.clone(),
        inputs: current_inputs.clone(),
        last_built_at: None,
        last_error: None,
    };
    if let Ok(Some(prior)) = load_lineage(workspace_root, resource_rel) {
        building.last_built_at = prior.last_built_at;
    }
    save_lineage(workspace_root, &building)?;

    match runner.run(&builder_path) {
        Ok(output) if output.exit_code == 0 => {
            // Re-hash after the build in case the task also touched inputs.
            let inputs = hash_inputs(workspace_root, &derived_path, &manifest)?;
            let lineage = DerivedLineage {
                resource_path: normalize_rel(resource_rel),
                state: DerivedState::Current,
                builder_task: builder_rel,
                output: output_rel,
                inputs,
                last_built_at: Some(now_iso()),
                last_error: None,
            };
            save_lineage(workspace_root, &lineage)?;
            load_derived_status(workspace_root, resource_rel)
        }
        Ok(output) => {
            let message = if output.stderr.trim().is_empty() {
                format!("builder exited with code {}", output.exit_code)
            } else {
                output.stderr.trim().to_string()
            };
            let lineage = DerivedLineage {
                resource_path: normalize_rel(resource_rel),
                state: DerivedState::Failed,
                builder_task: builder_rel,
                output: output_rel,
                inputs: current_inputs,
                last_built_at: building.last_built_at,
                last_error: Some(message),
            };
            save_lineage(workspace_root, &lineage)?;
            load_derived_status(workspace_root, resource_rel)
        }
        Err(err) => {
            let lineage = DerivedLineage {
                resource_path: normalize_rel(resource_rel),
                state: DerivedState::Failed,
                builder_task: builder_rel,
                output: output_rel,
                inputs: current_inputs,
                last_built_at: building.last_built_at,
                last_error: Some(err.to_string()),
            };
            save_lineage(workspace_root, &lineage)?;
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
        assert!(status.inputs.is_empty());
        assert!(status
            .current_inputs
            .iter()
            .any(|i| i.path.ends_with("input.txt") && i.hash.is_some()));
        assert!(status
            .current_inputs
            .iter()
            .any(|i| i.path.ends_with("src/a.txt") && i.hash.is_some()));

        let lineage = DerivedLineage {
            resource_path: rel.into(),
            state: DerivedState::Current,
            builder_task: "Reports/Build Summary.task".into(),
            output: "Reports/dist/index.html".into(),
            inputs: status.current_inputs.clone(),
            last_built_at: Some("2026-01-01T00:00:00Z".into()),
            last_error: None,
        };
        save_lineage(root, &lineage).unwrap();

        let current = load_derived_status(root, rel).unwrap();
        assert_eq!(current.state, DerivedState::Current);

        fs::write(root.join("Reports/input.txt"), "changed\n").unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
    }

    #[test]
    fn rebuild_marks_current_with_fake_uv() {
        let dir = fixture_workspace();
        let root = dir.path();
        let rel = "Reports/Summary.derived.yaml";

        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let python_path = bin.join("python");
        let out = root.join("Reports/dist/index.html");
        let uv_script = format!(
            r#"#!/bin/sh
if [ "$1" = "python" ] && [ "$2" = "find" ]; then
  printf '%s\n' '{python}'
  exit 0
fi
mkdir -p "$(dirname '{out}')"
printf 'built\n' > '{out}'
exit 0
"#,
            python = python_path.display(),
            out = out.display()
        );
        write_executable(&bin.join("uv"), &uv_script);
        write_executable(&python_path, "#!/bin/sh\nexit 0\n");

        let path = std::env::join_paths([bin.as_path(), Path::new("/bin"), Path::new("/usr/bin")])
            .unwrap();
        let runner = TaskRunner::with_env(EnvProvider::with_path(path));

        let before = load_derived_status(root, rel).unwrap();
        assert_eq!(before.state, DerivedState::Stale);

        let after = rebuild_derived(root, rel, &runner).unwrap();
        assert_eq!(after.state, DerivedState::Current);
        assert!(after.last_built_at.is_some());
        assert_eq!(after.builder_task, "Reports/Build Summary.task");
        assert!(!after.inputs.is_empty());
        assert!(out.is_file());

        fs::write(root.join("Reports/input.txt"), "again\n").unwrap();
        let stale = load_derived_status(root, rel).unwrap();
        assert_eq!(stale.state, DerivedState::Stale);
    }
}
