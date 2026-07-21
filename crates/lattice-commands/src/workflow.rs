//! Parse and execute `*.workflow.yaml` automation resources (bounded v1).
//!
//! v1 supports manual / resource.changed / form.submitted triggers and
//! `task.run`, `proposal.create`, and log-only `notification` steps.
//! Cron, durable daemon jobs, and a visual editor are out of scope.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use lattice_core::OPERATIONAL_DIR;
use serde::{Deserialize, Serialize};

use crate::contracts::{
    ExecutionResult, ExecutionStatus, ProposalSource, ProposalSourceType, TransactionProposal,
};
use crate::proposal::{create_proposal, proposal_now_iso};
use crate::task::{TaskError, TaskRunner};
use crate::Command;

pub const WORKFLOW_FORMAT: &str = "lattice-workflow";
pub const SUPPORTED_VERSION: u32 = 1;
pub const WORKFLOWS_DIR: &str = "workflows";
pub const WORKFLOW_RUNS_DIR: &str = "runs";

/// Errors from loading or running a Lattice workflow.
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    /// YAML failed structural validation after parse.
    #[error("invalid workflow at {path}: {message}")]
    Invalid { path: PathBuf, message: String },

    /// YAML parse failure.
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// I/O while reading or writing workflow artifacts.
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A step failed (task, proposal, or validation).
    #[error("workflow step `{step_id}` failed: {message}")]
    StepFailed { step_id: String, message: String },

    /// Execution was cancelled between steps.
    #[error("workflow cancelled")]
    Cancelled,

    /// Nested task failure.
    #[error(transparent)]
    Task(#[from] TaskError),

    /// Nested command/proposal store failure.
    #[error(transparent)]
    Commands(#[from] crate::Error),
}

pub type WorkflowResult<T> = std::result::Result<T, WorkflowError>;

fn default_enabled() -> bool {
    true
}

/// Parsed `*.workflow.yaml` document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowManifest {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub trigger: WorkflowTrigger,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
}

/// v1 trigger kinds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowTrigger {
    Manual,
    #[serde(rename = "resource.changed")]
    ResourceChanged {
        /// Workspace-relative path globs (`*`, `**`, `?`).
        #[serde(default)]
        paths: Vec<String>,
    },
    #[serde(rename = "form.submitted")]
    FormSubmitted {
        /// Workspace-relative path to a `.form.yaml` (or package-relative form file).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        form: Option<String>,
        /// Data package path when matching by package + form id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        package: Option<String>,
        /// Form id/name within `package` when `form` is not a full path.
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "form_id")]
        form_id: Option<String>,
    },
}

/// One ordered workflow step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub action: String,
    #[serde(default, rename = "with")]
    pub with: serde_yaml::Value,
}

/// Parameters for `task.run`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRunParams {
    pub task: String,
}

/// Parameters for `proposal.create`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalCreateParams {
    pub summary: String,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Parameters for log-only `notification`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationParams {
    #[serde(default)]
    pub message: String,
}

/// Per-step outcome captured in run history / UI logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepResult {
    pub id: String,
    pub action: String,
    pub status: ExecutionStatus,
    #[serde(default)]
    pub log: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
}

/// Full workflow run record (execution + step detail + provenance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunRecord {
    pub workflow_path: String,
    pub trigger: String,
    pub execution: ExecutionResult,
    pub steps: Vec<WorkflowStepResult>,
}

impl WorkflowManifest {
    /// Load and validate a workflow YAML file.
    pub fn load(path: &Path) -> WorkflowResult<Self> {
        let text = fs::read_to_string(path).map_err(|source| WorkflowError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(path, &text)
    }

    /// Parse YAML text with validation (path used only for error context).
    pub fn parse(path: &Path, text: &str) -> WorkflowResult<Self> {
        let manifest: WorkflowManifest =
            serde_yaml::from_str(text).map_err(|source| WorkflowError::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    fn check(&self, path: &Path) -> WorkflowResult<()> {
        let invalid = |message: String| WorkflowError::Invalid {
            path: path.to_path_buf(),
            message,
        };
        if self.format != WORKFLOW_FORMAT {
            return Err(invalid(format!(
                "expected format {WORKFLOW_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version == 0 || self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "workflow version {} is not supported (expected 1..={SUPPORTED_VERSION})",
                self.version
            )));
        }
        if self.name.trim().is_empty() {
            return Err(invalid("name must be non-empty".into()));
        }
        match &self.trigger {
            WorkflowTrigger::Manual => {}
            WorkflowTrigger::ResourceChanged { paths } => {
                if paths.is_empty() {
                    return Err(invalid(
                        "resource.changed trigger requires a non-empty paths list".into(),
                    ));
                }
            }
            WorkflowTrigger::FormSubmitted {
                form,
                package,
                form_id,
            } => {
                let has_form_path = form.as_ref().is_some_and(|value| !value.trim().is_empty());
                let has_package_form = package.as_ref().is_some_and(|value| !value.trim().is_empty())
                    && (form_id.as_ref().is_some_and(|value| !value.trim().is_empty())
                        || form.as_ref().is_some_and(|value| !value.trim().is_empty()));
                if !has_form_path && !has_package_form {
                    return Err(invalid(
                        "form.submitted trigger requires `form` path and/or `package` + form id"
                            .into(),
                    ));
                }
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for step in &self.steps {
            if step.id.trim().is_empty() {
                return Err(invalid("step id must be non-empty".into()));
            }
            if !seen.insert(step.id.clone()) {
                return Err(invalid(format!("duplicate step id {:?}", step.id)));
            }
            match step.action.as_str() {
                "task.run" | "proposal.create" | "notification" => {}
                other => {
                    return Err(invalid(format!(
                        "unknown step action {other:?} (supported: task.run, proposal.create, notification)"
                    )));
                }
            }
            // Fail fast on malformed `with` blocks for known actions.
            match step.action.as_str() {
                "task.run" => {
                    let _: TaskRunParams = deserialize_with(&step.with, path, &step.id)?;
                }
                "proposal.create" => {
                    let _: ProposalCreateParams = deserialize_with(&step.with, path, &step.id)?;
                }
                "notification" => {
                    let _: NotificationParams = deserialize_with(&step.with, path, &step.id)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Whether this workflow should react to a resource path change.
    pub fn matches_resource_change(&self, changed_path: &str) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.trigger {
            WorkflowTrigger::ResourceChanged { paths } => {
                paths.iter().any(|pattern| path_matches_glob(changed_path, pattern))
            }
            _ => false,
        }
    }

    /// Whether this workflow should react to a form submission.
    pub fn matches_form_submitted(
        &self,
        package_path: &str,
        form_name: &str,
        form_file_path: Option<&str>,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        let WorkflowTrigger::FormSubmitted {
            form,
            package,
            form_id,
        } = &self.trigger
        else {
            return false;
        };

        if let Some(form_path) = form.as_ref().filter(|value| value.contains('/') || value.contains('\\'))
        {
            if let Some(submitted) = form_file_path {
                if normalize_rel(submitted) == normalize_rel(form_path) {
                    return true;
                }
            }
            // Also accept package-relative form file naming conventions.
            let expected = format!(
                "{}/forms/{}.form.yaml",
                normalize_rel(package_path).trim_end_matches('/'),
                form_name
            );
            if normalize_rel(form_path) == expected {
                return true;
            }
        }

        let package_ok = package
            .as_ref()
            .map(|value| normalize_rel(value) == normalize_rel(package_path))
            .unwrap_or(false);
        let id = form_id
            .as_deref()
            .or_else(|| {
                form.as_deref()
                    .filter(|value| !value.contains('/') && !value.contains('\\'))
            })
            .unwrap_or("");
        package_ok && !id.is_empty() && id == form_name
    }
}

fn deserialize_with<T: for<'de> Deserialize<'de>>(
    value: &serde_yaml::Value,
    path: &Path,
    step_id: &str,
) -> WorkflowResult<T> {
    serde_yaml::from_value(value.clone()).map_err(|source| WorkflowError::Invalid {
        path: path.to_path_buf(),
        message: format!("step `{step_id}` has invalid `with` block: {source}"),
    })
}

fn normalize_rel(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

/// Simple glob matcher: `*` (within a path segment), `**` (cross-segment), `?` (one char).
pub fn path_matches_glob(path: &str, pattern: &str) -> bool {
    let path = normalize_rel(path);
    let pattern = normalize_rel(pattern);
    match_glob(&path, &pattern)
}

fn match_glob(path: &str, pattern: &str) -> bool {
    match_glob_chars(path.as_bytes(), pattern.as_bytes())
}

fn match_glob_chars(path: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern.starts_with(b"**/") {
        // Match zero or more path segments.
        if match_glob_chars(path, &pattern[3..]) {
            return true;
        }
        for (index, byte) in path.iter().enumerate() {
            if *byte == b'/' && match_glob_chars(&path[index + 1..], pattern) {
                return true;
            }
        }
        return match_glob_chars(path, &pattern[3..]);
    }
    if pattern == b"**" {
        return true;
    }
    if pattern.starts_with(b"**") {
        return match_glob_chars(path, &pattern[2..]);
    }
    if pattern[0] == b'*' {
        // Single-segment wildcard: consume until `/` or end.
        let mut index = 0;
        loop {
            if match_glob_chars(&path[index..], &pattern[1..]) {
                return true;
            }
            if index >= path.len() || path[index] == b'/' {
                return false;
            }
            index += 1;
        }
    }
    if path.is_empty() {
        return false;
    }
    if pattern[0] == b'?' || pattern[0] == path[0] {
        return match_glob_chars(&path[1..], &pattern[1..]);
    }
    false
}

/// Directory holding workflow run history: `<workspace>/.lattice/workflows/`.
pub fn workflows_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(OPERATIONAL_DIR).join(WORKFLOWS_DIR)
}

/// Directory for individual run JSON files.
pub fn workflow_runs_dir(workspace_root: &Path) -> PathBuf {
    workflows_dir(workspace_root).join(WORKFLOW_RUNS_DIR)
}

/// Persist a completed (or in-progress) run record.
pub fn save_workflow_run(workspace_root: &Path, record: &WorkflowRunRecord) -> WorkflowResult<()> {
    let dir = workflow_runs_dir(workspace_root);
    fs::create_dir_all(&dir).map_err(|source| WorkflowError::Io {
        path: dir.clone(),
        source,
    })?;
    let path = dir.join(format!("{}.json", record.execution.id));
    let payload = serde_json::to_string_pretty(record).map_err(|err| WorkflowError::Invalid {
        path: path.clone(),
        message: format!("failed to serialize run record: {err}"),
    })?;
    fs::write(&path, payload).map_err(|source| WorkflowError::Io { path, source })?;
    Ok(())
}

/// List recent workflow runs for a workflow path (newest first), capped.
pub fn list_workflow_runs(
    workspace_root: &Path,
    workflow_path: &str,
    limit: usize,
) -> WorkflowResult<Vec<WorkflowRunRecord>> {
    let dir = workflow_runs_dir(workspace_root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let target = normalize_rel(workflow_path);
    let mut runs = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|source| WorkflowError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| WorkflowError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let payload = fs::read_to_string(&path).map_err(|source| WorkflowError::Io {
            path: path.clone(),
            source,
        })?;
        let record: WorkflowRunRecord =
            serde_json::from_str(&payload).map_err(|err| WorkflowError::Invalid {
                path,
                message: format!("invalid run record: {err}"),
            })?;
        if normalize_rel(&record.workflow_path) == target {
            runs.push(record);
        }
    }
    runs.sort_by(|left, right| {
        right
            .execution
            .started_at
            .cmp(&left.execution.started_at)
            .then_with(|| right.execution.id.cmp(&left.execution.id))
    });
    if runs.len() > limit {
        runs.truncate(limit);
    }
    Ok(runs)
}

/// Update `enabled` in a workflow YAML file (preserves unrelated keys via rewrite).
pub fn set_workflow_enabled(path: &Path, enabled: bool) -> WorkflowResult<WorkflowManifest> {
    let text = fs::read_to_string(path).map_err(|source| WorkflowError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut manifest = WorkflowManifest::parse(path, &text)?;
    manifest.enabled = enabled;
    let rewritten = serde_yaml::to_string(&manifest).map_err(|source| WorkflowError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, rewritten).map_err(|source| WorkflowError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(manifest)
}

/// Discover `*.workflow.yaml` / `*.workflow.yml` under a workspace root.
pub fn discover_workflows(workspace_root: &Path) -> WorkflowResult<Vec<(PathBuf, WorkflowManifest)>> {
    let mut found = Vec::new();
    discover_workflows_in(workspace_root, workspace_root, &mut found)?;
    Ok(found)
}

fn discover_workflows_in(
    workspace_root: &Path,
    dir: &Path,
    out: &mut Vec<(PathBuf, WorkflowManifest)>,
) -> WorkflowResult<()> {
    let entries = fs::read_dir(dir).map_err(|source| WorkflowError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| WorkflowError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == OPERATIONAL_DIR || name == ".git" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            discover_workflows_in(workspace_root, &path, out)?;
            continue;
        }
        if name.ends_with(".workflow.yaml") || name.ends_with(".workflow.yml") {
            match WorkflowManifest::load(&path) {
                Ok(manifest) => out.push((path, manifest)),
                Err(err) => {
                    // Skip invalid workflows during discovery; callers may log.
                    eprintln!("lattice: skip invalid workflow {}: {err}", path.display());
                }
            }
        }
    }
    Ok(())
}

fn now_iso() -> String {
    proposal_now_iso()
}

fn trigger_label(trigger: &WorkflowTrigger) -> &'static str {
    match trigger {
        WorkflowTrigger::Manual => "manual",
        WorkflowTrigger::ResourceChanged { .. } => "resource.changed",
        WorkflowTrigger::FormSubmitted { .. } => "form.submitted",
    }
}

/// Resolve a workflow-relative or workspace-relative path against the workspace root.
pub fn resolve_workspace_path(workspace_root: &Path, workflow_path: &Path, rel: &str) -> PathBuf {
    let candidate = Path::new(rel);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    let from_workflow = workflow_path
        .parent()
        .unwrap_or(workspace_root)
        .join(rel);
    if from_workflow.exists() {
        return from_workflow;
    }
    workspace_root.join(rel)
}

/// Execute a workflow to completion (or until cancel / first failing step).
pub fn run_workflow(
    workspace_root: &Path,
    workflow_path: &Path,
    manifest: &WorkflowManifest,
    trigger: &str,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<WorkflowRunRecord> {
    let execution_id = uuid::Uuid::now_v7().to_string();
    let started_at = now_iso();
    let workflow_rel = workflow_path
        .strip_prefix(workspace_root)
        .unwrap_or(workflow_path)
        .to_string_lossy()
        .replace('\\', "/");

    let mut step_results = Vec::new();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut proposal_id = None;
    let mut status = ExecutionStatus::Succeeded;

    let runner = TaskRunner::new();

    for step in &manifest.steps {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            status = ExecutionStatus::Cancelled;
            stderr.push_str("workflow cancelled\n");
            break;
        }

        let mut step_log = String::new();
        let step_status;
        let mut step_proposal = None;

        match step.action.as_str() {
            "task.run" => {
                let params: TaskRunParams = deserialize_with(&step.with, workflow_path, &step.id)?;
                let task_path =
                    resolve_workspace_path(workspace_root, workflow_path, &params.task);
                step_log.push_str(&format!("task.run {}\n", task_path.display()));
                match runner.run(&task_path) {
                    Ok(out) => {
                        step_log.push_str(&out.stdout);
                        if !out.stderr.is_empty() {
                            step_log.push_str(&out.stderr);
                        }
                        if out.exit_code == 0 {
                            step_status = ExecutionStatus::Succeeded;
                            stdout.push_str(&format!("[{}] ok (exit 0)\n", step.id));
                        } else {
                            step_status = ExecutionStatus::Failed;
                            status = ExecutionStatus::Failed;
                            let msg = format!(
                                "task exited with code {} at {}",
                                out.exit_code,
                                task_path.display()
                            );
                            stderr.push_str(&format!("[{}] {msg}\n", step.id));
                            step_results.push(WorkflowStepResult {
                                id: step.id.clone(),
                                action: step.action.clone(),
                                status: step_status,
                                log: step_log,
                                proposal_id: None,
                            });
                            break;
                        }
                    }
                    Err(TaskError::TimedOut {
                        timeout_seconds,
                        stdout: task_stdout,
                        stderr: task_stderr,
                    }) => {
                        step_log.push_str(&task_stdout);
                        step_log.push_str(&task_stderr);
                        step_status = ExecutionStatus::Failed;
                        status = ExecutionStatus::Failed;
                        stderr.push_str(&format!(
                            "[{}] task timed out after {timeout_seconds}s\n",
                            step.id
                        ));
                        step_results.push(WorkflowStepResult {
                            id: step.id.clone(),
                            action: step.action.clone(),
                            status: step_status,
                            log: step_log,
                            proposal_id: None,
                        });
                        break;
                    }
                    Err(err) => {
                        step_status = ExecutionStatus::Failed;
                        status = ExecutionStatus::Failed;
                        let msg = err.to_string();
                        step_log.push_str(&msg);
                        stderr.push_str(&format!("[{}] {msg}\n", step.id));
                        step_results.push(WorkflowStepResult {
                            id: step.id.clone(),
                            action: step.action.clone(),
                            status: step_status,
                            log: step_log,
                            proposal_id: None,
                        });
                        break;
                    }
                }
            }
            "proposal.create" => {
                let params: ProposalCreateParams =
                    deserialize_with(&step.with, workflow_path, &step.id)?;
                let created = create_proposal(
                    workspace_root,
                    TransactionProposal {
                        id: String::new(),
                        source: ProposalSource {
                            source_type: ProposalSourceType::Workflow,
                            resource: Some(workflow_rel.clone()),
                        },
                        summary: params.summary,
                        commands: params.commands,
                        affected_paths: params.affected_paths,
                        warnings: params.warnings,
                        created_at: String::new(),
                        status: Default::default(),
                    },
                )?;
                step_proposal = Some(created.id.clone());
                proposal_id = Some(created.id.clone());
                step_log.push_str(&format!("created proposal {}\n", created.id));
                stdout.push_str(&format!("[{}] proposal {}\n", step.id, created.id));
                step_status = ExecutionStatus::Succeeded;
            }
            "notification" => {
                let params: NotificationParams =
                    deserialize_with(&step.with, workflow_path, &step.id)?;
                let message = if params.message.is_empty() {
                    format!("notification from step {}", step.id)
                } else {
                    params.message
                };
                step_log.push_str(&message);
                step_log.push('\n');
                stdout.push_str(&format!("[{}] {message}\n", step.id));
                // Log-only: also surface via eprintln for daemon/desktop logs.
                eprintln!("lattice workflow notification [{}]: {message}", step.id);
                step_status = ExecutionStatus::Succeeded;
            }
            other => {
                return Err(WorkflowError::Invalid {
                    path: workflow_path.to_path_buf(),
                    message: format!("unknown step action {other:?}"),
                });
            }
        }

        step_results.push(WorkflowStepResult {
            id: step.id.clone(),
            action: step.action.clone(),
            status: step_status,
            log: step_log,
            proposal_id: step_proposal,
        });
    }

    if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst))
        && status == ExecutionStatus::Succeeded
    {
        status = ExecutionStatus::Cancelled;
    }

    let record = WorkflowRunRecord {
        workflow_path: workflow_rel,
        trigger: trigger.to_string(),
        execution: ExecutionResult {
            id: execution_id,
            status,
            stdout,
            stderr,
            started_at,
            finished_at: Some(now_iso()),
            outputs: Vec::new(),
            proposal_id,
        },
        steps: step_results,
    };
    save_workflow_run(workspace_root, &record)?;
    Ok(record)
}

/// Convenience: load + run with a trigger label derived from the manifest.
pub fn load_and_run_workflow(
    workspace_root: &Path,
    workflow_path: &Path,
    trigger_override: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<WorkflowRunRecord> {
    let manifest = WorkflowManifest::load(workflow_path)?;
    let trigger = trigger_override.unwrap_or_else(|| trigger_label(&manifest.trigger));
    run_workflow(workspace_root, workflow_path, &manifest, trigger, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn parses_simple_manual_workflow() {
        let path = fixture("Simple.workflow.yaml");
        let manifest = WorkflowManifest::load(&path).expect("load");
        assert_eq!(manifest.format, WORKFLOW_FORMAT);
        assert_eq!(manifest.version, 1);
        assert!(manifest.enabled);
        assert!(matches!(manifest.trigger, WorkflowTrigger::Manual));
        assert_eq!(manifest.steps.len(), 3);
        assert_eq!(manifest.steps[0].action, "task.run");
        assert_eq!(manifest.steps[1].action, "proposal.create");
        assert_eq!(manifest.steps[2].action, "notification");
    }

    #[test]
    fn rejects_unknown_action() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Bad
trigger:
  type: manual
steps:
  - id: x
    action: page.create-from-template
    with: {}
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(err.to_string().contains("unknown step action"));
    }

    #[test]
    fn rejects_unknown_trigger() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Bad
trigger:
  type: schedule.cron
  expression: "0 * * * *"
steps: []
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(
            err.to_string().contains("failed to parse")
                || err.to_string().contains("unknown")
                || err.to_string().contains("did not match"),
            "{err}"
        );
    }

    #[test]
    fn glob_matching_supports_star_and_double_star() {
        assert!(path_matches_glob("Notes/A.md", "Notes/*"));
        assert!(path_matches_glob("Notes/deep/A.md", "Notes/**"));
        assert!(path_matches_glob("Notes/deep/A.md", "Notes/**/*.md"));
        assert!(!path_matches_glob("Other/A.md", "Notes/*"));
        assert!(path_matches_glob("Data/x.csv", "Data/*.csv"));
    }

    #[test]
    fn enabled_false_skips_resource_and_form_match() {
        let mut manifest = WorkflowManifest {
            format: WORKFLOW_FORMAT.into(),
            version: 1,
            name: "Off".into(),
            enabled: false,
            trigger: WorkflowTrigger::ResourceChanged {
                paths: vec!["Notes/**".into()],
            },
            steps: vec![],
        };
        assert!(!manifest.matches_resource_change("Notes/A.md"));
        manifest.trigger = WorkflowTrigger::FormSubmitted {
            form: None,
            package: Some("CRM.data".into()),
            form_id: Some("Intake".into()),
        };
        assert!(!manifest.matches_form_submitted("CRM.data", "Intake", None));
    }

    #[test]
    fn run_creates_proposal_with_workflow_source() {
        let dir = tempfile::tempdir().unwrap();
        // Minimal workspace marker so proposal paths are under a root.
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("Simple.workflow.yaml");
        // Proposal-only workflow (no uv) for hermetic CI.
        let yaml = r##"
format: lattice-workflow
version: 1
name: Proposal only
trigger:
  type: manual
steps:
  - id: propose
    action: proposal.create
    with:
      summary: Create from workflow
      commands:
        - type: page-create
          path: Notes/FromWorkflow.md
          content: "# From workflow\n"
  - id: note
    action: notification
    with:
      message: proposal created
"##;
        fs::write(&workflow_path, yaml).unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Succeeded);
        let proposal_id = record.execution.proposal_id.expect("proposal id");
        let proposal = crate::load_proposal(dir.path(), &proposal_id).unwrap();
        assert_eq!(proposal.source.source_type, ProposalSourceType::Workflow);
        assert_eq!(
            proposal.source.resource.as_deref(),
            Some("Simple.workflow.yaml")
        );
        assert_eq!(record.steps.len(), 2);
        assert!(workflow_runs_dir(dir.path()).join(format!("{}.json", record.execution.id)).is_file());
    }

    #[test]
    fn set_enabled_rewrites_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Toggle.workflow.yaml");
        fs::write(
            &path,
            r#"
format: lattice-workflow
version: 1
name: Toggle
enabled: true
trigger:
  type: manual
steps: []
"#,
        )
        .unwrap();
        let updated = set_workflow_enabled(&path, false).unwrap();
        assert!(!updated.enabled);
        let reloaded = WorkflowManifest::load(&path).unwrap();
        assert!(!reloaded.enabled);
    }

    #[test]
    fn form_submitted_matches_package_and_id() {
        let manifest = WorkflowManifest {
            format: WORKFLOW_FORMAT.into(),
            version: 1,
            name: "Form".into(),
            enabled: true,
            trigger: WorkflowTrigger::FormSubmitted {
                form: Some("ContactIntake".into()),
                package: Some("Data/CRM.data".into()),
                form_id: None,
            },
            steps: vec![],
        };
        assert!(manifest.matches_form_submitted("Data/CRM.data", "ContactIntake", None));
        assert!(!manifest.matches_form_submitted("Data/Other.data", "ContactIntake", None));
    }
}
