//! Parse and execute `*.workflow.yaml` automation resources (bounded v1).
//!
//! v1 supports manual / resource.changed / form.submitted / schedule triggers and
//! `task.run`, `proposal.create`, and log-only `notification` steps.
//! Optional per-step `retry` (max attempts + backoff) and `parallel` child groups
//! (bounded concurrent fan-out, then join) are supported by the runner.
//! Interval schedule firing is owned by `latticed` (see daemon schedule runner).
//! Cron-only schedules are parsed/validated but not evaluated yet.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lattice_core::OPERATIONAL_DIR;
use serde::{Deserialize, Serialize};

use crate::contracts::{
    ExecutionResult, ExecutionStatus, ProposalSource, ProposalSourceType, TransactionProposal,
};
use crate::proposal::{create_proposal, proposal_now_iso};
use crate::task::{TaskError, TaskManifest, TaskRunner, TASK_MANIFEST_FILENAME};
use crate::Command;

pub const WORKFLOW_FORMAT: &str = "lattice-workflow";
pub const SUPPORTED_VERSION: u32 = 1;
pub const WORKFLOWS_DIR: &str = "workflows";
pub const WORKFLOW_RUNS_DIR: &str = "runs";
/// Max concurrent children inside one `parallel` group (extra children run in waves).
pub const MAX_PARALLEL_STEPS: usize = 8;

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
    /// Time-based trigger. Interval firing is handled by `latticed`; cron-only
    /// expressions are accepted at parse time but deferred until a cron evaluator
    /// ships (interval is preferred when both are set).
    ///
    /// Require at least one of `interval_seconds` or `cron`. Unknown fields fail closed.
    Schedule(ScheduleTrigger),
}

/// Fields for `type: schedule`. Unknown keys are rejected at parse time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleTrigger {
    /// Fixed period in whole seconds (must be > 0 when set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    /// Cron expression (non-empty when set; 5- or 6-field forms accepted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// Optional IANA timezone name (non-empty when set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// Optional retry policy for a leaf step (total attempts including the first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStepRetry {
    /// Total attempts including the first try (`>= 1`).
    pub max_attempts: u32,
    /// Seconds to sleep after a failed attempt before the next try.
    #[serde(default)]
    pub backoff_seconds: u64,
}

/// One ordered workflow step (leaf action or parallel group).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    /// Leaf action (`task.run`, …). Empty or `"parallel"` when [`Self::parallel`] is set.
    #[serde(default)]
    pub action: String,
    #[serde(default, rename = "with")]
    pub with: serde_yaml::Value,
    /// Optional retry for leaf steps (ignored on parallel groups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<WorkflowStepRetry>,
    /// Concurrent child steps; when non-empty this step is a parallel group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parallel: Vec<WorkflowStep>,
}

fn default_attempts() -> u32 {
    1
}

fn is_one_attempt(value: &u32) -> bool {
    *value <= 1
}

/// Parameters for `task.run`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRunParams {
    pub task: String,
    /// Acknowledge retrying a task that does not declare `execution.idempotent: true`.
    ///
    /// Required when the step sets `retry.max_attempts > 1` and the task is not
    /// marked idempotent; without this flag the runner rejects the retry policy.
    #[serde(default)]
    pub allow_unsafe_retry: bool,
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
    /// Attempts consumed (including the successful one, or all failed tries).
    #[serde(default = "default_attempts", skip_serializing_if = "is_one_attempt")]
    pub attempts: u32,
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
                let has_package_form = package
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty())
                    && (form_id
                        .as_ref()
                        .is_some_and(|value| !value.trim().is_empty())
                        || form.as_ref().is_some_and(|value| !value.trim().is_empty()));
                if !has_form_path && !has_package_form {
                    return Err(invalid(
                        "form.submitted trigger requires `form` path and/or `package` + form id"
                            .into(),
                    ));
                }
            }
            WorkflowTrigger::Schedule(schedule) => {
                validate_schedule_trigger(schedule, path)?;
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for step in &self.steps {
            validate_step(step, path, &mut seen, /* allow_parallel */ true)?;
        }
        Ok(())
    }

    /// Whether this workflow should react to a resource path change.
    pub fn matches_resource_change(&self, changed_path: &str) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.trigger {
            WorkflowTrigger::ResourceChanged { paths } => paths
                .iter()
                .any(|pattern| path_matches_glob(changed_path, pattern)),
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

        if let Some(form_path) = form
            .as_ref()
            .filter(|value| value.contains('/') || value.contains('\\'))
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

fn validate_step(
    step: &WorkflowStep,
    path: &Path,
    seen: &mut std::collections::BTreeSet<String>,
    allow_parallel: bool,
) -> WorkflowResult<()> {
    let invalid = |message: String| WorkflowError::Invalid {
        path: path.to_path_buf(),
        message,
    };
    if step.id.trim().is_empty() {
        return Err(invalid("step id must be non-empty".into()));
    }
    if !seen.insert(step.id.clone()) {
        return Err(invalid(format!("duplicate step id {:?}", step.id)));
    }
    if let Some(retry) = &step.retry {
        if retry.max_attempts == 0 {
            return Err(invalid(format!(
                "step `{}` retry.max_attempts must be >= 1",
                step.id
            )));
        }
    }
    if !step.parallel.is_empty() {
        if !allow_parallel {
            return Err(invalid(format!(
                "step `{}`: nested parallel groups are not supported",
                step.id
            )));
        }
        if !step.action.is_empty() && step.action != "parallel" {
            return Err(invalid(format!(
                "step `{}`: parallel groups must use action \"parallel\" or omit action (found {:?})",
                step.id, step.action
            )));
        }
        if step.retry.is_some() {
            return Err(invalid(format!(
                "step `{}`: retry applies to leaf steps only (put retry on parallel children)",
                step.id
            )));
        }
        if !with_is_empty(&step.with) {
            return Err(invalid(format!(
                "step `{}`: parallel groups must not set `with` (children carry their own params)",
                step.id
            )));
        }
        for child in &step.parallel {
            validate_step(child, path, seen, /* allow_parallel */ false)?;
        }
        return Ok(());
    }
    match step.action.as_str() {
        "task.run" | "proposal.create" | "notification" => {}
        "parallel" => {
            return Err(invalid(format!(
                "step `{}`: action \"parallel\" requires a non-empty `parallel` child list",
                step.id
            )));
        }
        "" => {
            return Err(invalid(format!(
                "step `{}`: action is required for leaf steps",
                step.id
            )));
        }
        other => {
            return Err(invalid(format!(
                "unknown step action {other:?} (supported: task.run, proposal.create, notification, parallel)"
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
    Ok(())
}

fn with_is_empty(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::Null => true,
        serde_yaml::Value::Mapping(map) => map.is_empty(),
        serde_yaml::Value::Sequence(seq) => seq.is_empty(),
        _ => false,
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

/// Fail-closed checks for `type: schedule` fields.
fn validate_schedule_trigger(schedule: &ScheduleTrigger, path: &Path) -> WorkflowResult<()> {
    let invalid = |message: String| WorkflowError::Invalid {
        path: path.to_path_buf(),
        message,
    };
    let cron_trimmed = schedule
        .cron
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let has_interval = schedule.interval_seconds.is_some();
    let has_cron = cron_trimmed.is_some();
    if !has_interval && !has_cron {
        return Err(invalid(
            "schedule trigger requires `interval_seconds` and/or non-empty `cron`".into(),
        ));
    }
    if let Some(seconds) = schedule.interval_seconds {
        if seconds == 0 {
            return Err(invalid(
                "schedule trigger `interval_seconds` must be greater than 0".into(),
            ));
        }
    }
    if schedule.cron.is_some() && cron_trimmed.is_none() {
        return Err(invalid(
            "schedule trigger `cron` must be a non-empty expression".into(),
        ));
    }
    if let Some(expression) = cron_trimmed {
        let fields = expression.split_whitespace().count();
        if fields != 5 && fields != 6 {
            return Err(invalid(format!(
                "schedule trigger `cron` must have 5 or 6 fields, found {fields}"
            )));
        }
    }
    if let Some(tz) = &schedule.timezone {
        if tz.trim().is_empty() {
            return Err(invalid(
                "schedule trigger `timezone` must be non-empty when set".into(),
            ));
        }
    }
    Ok(())
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

/// Load one workflow run by execution id.
pub fn load_workflow_run(
    workspace_root: &Path,
    execution_id: &str,
) -> WorkflowResult<WorkflowRunRecord> {
    let path = workflow_runs_dir(workspace_root).join(format!("{execution_id}.json"));
    let payload = fs::read_to_string(&path).map_err(|source| WorkflowError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&payload).map_err(|err| WorkflowError::Invalid {
        path,
        message: format!("invalid run record: {err}"),
    })
}

/// List recent workflow runs across all workflows (newest first), capped.
pub fn list_all_workflow_runs(
    workspace_root: &Path,
    limit: usize,
) -> WorkflowResult<Vec<WorkflowRunRecord>> {
    let dir = workflow_runs_dir(workspace_root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
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
        runs.push(record);
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

/// Mark stranded `running` run JSON as `abandoned` after a daemon restart.
///
/// Policy: any on-disk run still `running` cannot still be owned by this process,
/// so it is closed as abandoned (not cancelled — cancel implies an intentional stop).
pub fn reconcile_abandoned_workflow_runs(workspace_root: &Path) -> WorkflowResult<usize> {
    let runs = list_all_workflow_runs(workspace_root, usize::MAX)?;
    let mut marked = 0usize;
    let finished_at = now_iso();
    for mut record in runs {
        if record.execution.status != ExecutionStatus::Running {
            continue;
        }
        record.execution.status = ExecutionStatus::Abandoned;
        record.execution.finished_at = Some(finished_at.clone());
        if record.execution.stderr.is_empty() {
            record.execution.stderr =
                "abandoned: owning process exited before the run finished\n".into();
        } else if !record.execution.stderr.ends_with('\n') {
            record.execution.stderr.push('\n');
            record
                .execution
                .stderr
                .push_str("abandoned: owning process exited before the run finished\n");
        } else {
            record
                .execution
                .stderr
                .push_str("abandoned: owning process exited before the run finished\n");
        }
        save_workflow_run(workspace_root, &record)?;
        marked += 1;
    }
    Ok(marked)
}

/// Build a shared [`ExecutionSummary`] from a persisted run record.
pub fn execution_summary_from_record(
    workspace_root: &Path,
    record: &WorkflowRunRecord,
    cancel_owner: &str,
) -> crate::ExecutionSummary {
    let cancellable = record.execution.status == ExecutionStatus::Running
        && cancel_owner != crate::CANCEL_OWNER_NONE;
    let current_step_id = record
        .steps
        .iter()
        .rev()
        .find(|step| step.status == ExecutionStatus::Running)
        .or_else(|| record.steps.last())
        .map(|step| step.id.clone());
    let attempt = record.steps.last().map(|step| step.attempts);
    let mut proposal_ids = record.execution.proposal_ids.clone();
    if proposal_ids.is_empty() {
        if let Some(id) = &record.execution.proposal_id {
            proposal_ids.push(id.clone());
        }
    }
    for step in &record.steps {
        if let Some(id) = &step.proposal_id {
            if !proposal_ids.iter().any(|existing| existing == id) {
                proposal_ids.push(id.clone());
            }
        }
    }
    crate::ExecutionSummary {
        execution_id: record.execution.id.clone(),
        workspace_root: workspace_root.to_string_lossy().replace('\\', "/"),
        resource_path: record.workflow_path.clone(),
        kind: "workflow".into(),
        trigger: record.trigger.clone(),
        status: record.execution.status,
        started_at: record.execution.started_at.clone(),
        finished_at: record.execution.finished_at.clone(),
        current_step_id,
        attempt,
        proposal_ids,
        cancel_owner: cancel_owner.to_string(),
        cancellable,
    }
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
pub fn discover_workflows(
    workspace_root: &Path,
) -> WorkflowResult<Vec<(PathBuf, WorkflowManifest)>> {
    let mut found = Vec::new();
    discover_workflows_in(workspace_root, workspace_root, &mut found)?;
    Ok(found)
}

/// Enabled workflow with a `schedule` trigger, ready for daemon evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledWorkflow {
    pub path: PathBuf,
    pub manifest: WorkflowManifest,
}

impl ScheduledWorkflow {
    /// Borrow the schedule trigger fields.
    pub fn schedule(&self) -> &ScheduleTrigger {
        match &self.manifest.trigger {
            WorkflowTrigger::Schedule(schedule) => schedule,
            WorkflowTrigger::Manual
            | WorkflowTrigger::ResourceChanged { .. }
            | WorkflowTrigger::FormSubmitted { .. } => {
                unreachable!("ScheduledWorkflow requires a schedule trigger")
            }
        }
    }

    /// Workspace-relative workflow path using `/` separators.
    pub fn relative_path(&self, workspace_root: &Path) -> String {
        self.path
            .strip_prefix(workspace_root)
            .unwrap_or(&self.path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

/// Discover enabled workflows whose trigger is `type: schedule`.
///
/// Disabled workflows are omitted (manual Run still works via
/// [`load_and_run_workflow`]).
pub fn discover_scheduled_workflows(
    workspace_root: &Path,
) -> WorkflowResult<Vec<ScheduledWorkflow>> {
    let mut out = Vec::new();
    for (path, manifest) in discover_workflows(workspace_root)? {
        if !manifest.enabled {
            continue;
        }
        if matches!(manifest.trigger, WorkflowTrigger::Schedule(_)) {
            out.push(ScheduledWorkflow { path, manifest });
        }
    }
    Ok(out)
}

/// Result of evaluating whether a schedule trigger should fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleDue {
    /// Interval has elapsed (or there is no prior fire); run now.
    Due,
    /// Interval has not elapsed yet.
    NotDue,
    /// Cron-only schedule: accepted in YAML but not evaluated by this skeleton.
    ///
    /// TODO: add a lightweight cron evaluator (with optional IANA timezone) so
    /// cron-only workflows fire without requiring `interval_seconds`.
    CronDeferred,
}

/// Decide whether a schedule trigger is due at `now`.
///
/// Prefer `interval_seconds` when set (even if `cron` is also present). Cron-only
/// triggers return [`ScheduleDue::CronDeferred`] until an evaluator ships.
pub fn evaluate_schedule_due(
    schedule: &ScheduleTrigger,
    last_fire: Option<SystemTime>,
    now: SystemTime,
) -> ScheduleDue {
    if let Some(interval) = schedule.interval_seconds.filter(|seconds| *seconds > 0) {
        return match last_fire {
            None => ScheduleDue::Due,
            Some(last) => {
                let elapsed = now.duration_since(last).unwrap_or(Duration::ZERO);
                if elapsed.as_secs() >= interval {
                    ScheduleDue::Due
                } else {
                    ScheduleDue::NotDue
                }
            }
        };
    }
    if schedule
        .cron
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return ScheduleDue::CronDeferred;
    }
    ScheduleDue::NotDue
}

/// Latest `started_at` among persisted runs with trigger label `schedule`.
pub fn last_schedule_run_at(
    workspace_root: &Path,
    workflow_path: &str,
) -> WorkflowResult<Option<SystemTime>> {
    let runs = list_workflow_runs(workspace_root, workflow_path, 64)?;
    for run in runs {
        if run.trigger != "schedule" {
            continue;
        }
        if let Some(started) = parse_iso8601_z(&run.execution.started_at) {
            return Ok(Some(started));
        }
    }
    Ok(None)
}

/// Parse UTC timestamps produced by [`crate::proposal_now_iso`] (`YYYY-MM-DDTHH:MM:SSZ`).
fn parse_iso8601_z(value: &str) -> Option<SystemTime> {
    let value = value.trim();
    let (date, rest) = value.split_once('T')?;
    let time = rest.strip_suffix('Z').or_else(|| rest.strip_suffix('z'))?;
    let time = time.split('.').next()?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() {
        return None;
    }
    let mut time_parts = time.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second: u32 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() {
        return None;
    }
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
    if secs < 0 {
        return None;
    }
    Some(UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// Howard Hinnant `days_from_civil` (proleptic Gregorian) — inverse of proposal ISO helper.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era) * 146_097 + i64::from(doe) - 719_468
}

#[allow(clippy::only_used_in_recursion)]
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
        WorkflowTrigger::Schedule(_) => "schedule",
    }
}

/// Resolve a workflow-relative or workspace-relative path against the workspace root.
pub fn resolve_workspace_path(workspace_root: &Path, workflow_path: &Path, rel: &str) -> PathBuf {
    let candidate = Path::new(rel);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    let from_workflow = workflow_path.parent().unwrap_or(workspace_root).join(rel);
    if from_workflow.exists() {
        return from_workflow;
    }
    workspace_root.join(rel)
}

struct LeafOutcome {
    status: ExecutionStatus,
    log: String,
    stdout: String,
    stderr: String,
    proposal_ids: Vec<String>,
}

struct StepBatchOutcome {
    /// Child results (declaration order) plus optional parallel-group summary.
    results: Vec<WorkflowStepResult>,
    status: ExecutionStatus,
    stdout: String,
    stderr: String,
    proposal_ids: Vec<String>,
}

fn retry_budget(retry: Option<&WorkflowStepRetry>) -> (u32, u64) {
    match retry {
        Some(policy) => (policy.max_attempts.max(1), policy.backoff_seconds),
        None => (1, 0),
    }
}

fn extend_proposal_ids(into: &mut Vec<String>, from: impl IntoIterator<Item = String>) {
    for id in from {
        if !into.contains(&id) {
            into.push(id);
        }
    }
}

fn primary_proposal_id(ids: &[String]) -> Option<String> {
    ids.first().cloned()
}

/// Sleep up to `backoff_seconds`, returning early when cancel is set.
/// Returns `true` when cancelled during the wait.
fn interruptible_backoff(backoff_seconds: u64, cancel: Option<&AtomicBool>) -> bool {
    if backoff_seconds == 0 {
        return cancel.is_some_and(|flag| flag.load(Ordering::SeqCst));
    }
    let deadline = Instant::now() + Duration::from_secs(backoff_seconds);
    while Instant::now() < deadline {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(remaining.min(Duration::from_millis(50)));
    }
    cancel.is_some_and(|flag| flag.load(Ordering::SeqCst))
}

fn task_package_manifest(
    workspace_root: &Path,
    workflow_path: &Path,
    task_rel: &str,
) -> WorkflowResult<(PathBuf, TaskManifest)> {
    let task_path = resolve_workspace_path(workspace_root, workflow_path, task_rel);
    let package_dir = if task_path.is_file() {
        task_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(task_path.clone())
    } else {
        task_path.clone()
    };
    let manifest_path = if package_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == TASK_MANIFEST_FILENAME)
    {
        package_dir.clone()
    } else {
        package_dir.join(TASK_MANIFEST_FILENAME)
    };
    let manifest = TaskManifest::load(&manifest_path)?;
    Ok((task_path, manifest))
}

fn reject_unsafe_task_retry(
    workspace_root: &Path,
    workflow_path: &Path,
    step: &WorkflowStep,
    max_attempts: u32,
) -> WorkflowResult<Option<LeafOutcome>> {
    if max_attempts <= 1 || step.action != "task.run" {
        return Ok(None);
    }
    let params: TaskRunParams = deserialize_with(&step.with, workflow_path, &step.id)?;
    if params.allow_unsafe_retry {
        return Ok(None);
    }
    let (_, manifest) = match task_package_manifest(workspace_root, workflow_path, &params.task) {
        Ok(value) => value,
        // Missing/invalid packages fail inside the attempt loop with the usual errors.
        Err(_) => return Ok(None),
    };
    if manifest.execution.idempotent {
        return Ok(None);
    }
    let msg = format!(
        "task {:?} is not marked execution.idempotent; refusing retry \
         (max_attempts={max_attempts}) without with.allow_unsafe_retry: true",
        params.task
    );
    Ok(Some(LeafOutcome {
        status: ExecutionStatus::Failed,
        log: format!("{msg}\n"),
        stdout: String::new(),
        stderr: format!("[{}] {msg}\n", step.id),
        proposal_ids: Vec::new(),
    }))
}

fn execute_leaf_once(
    workspace_root: &Path,
    workflow_path: &Path,
    workflow_rel: &str,
    execution_id: &str,
    step: &WorkflowStep,
    runner: &TaskRunner,
) -> WorkflowResult<LeafOutcome> {
    let mut log = String::new();
    let mut stdout = String::new();
    let mut stderr = String::new();
    match step.action.as_str() {
        "task.run" => {
            let params: TaskRunParams = deserialize_with(&step.with, workflow_path, &step.id)?;
            let task_path = resolve_workspace_path(workspace_root, workflow_path, &params.task);
            log.push_str(&format!("task.run {}\n", task_path.display()));
            match runner.run(&task_path) {
                Ok(out) => {
                    log.push_str(&out.stdout);
                    if !out.stderr.is_empty() {
                        log.push_str(&out.stderr);
                    }
                    if out.exit_code == 0 {
                        stdout.push_str(&format!("[{}] ok (exit 0)\n", step.id));
                        Ok(LeafOutcome {
                            status: ExecutionStatus::Succeeded,
                            log,
                            stdout,
                            stderr,
                            proposal_ids: Vec::new(),
                        })
                    } else {
                        let msg = format!(
                            "task exited with code {} at {}",
                            out.exit_code,
                            task_path.display()
                        );
                        stderr.push_str(&format!("[{}] {msg}\n", step.id));
                        Ok(LeafOutcome {
                            status: ExecutionStatus::Failed,
                            log,
                            stdout,
                            stderr,
                            proposal_ids: Vec::new(),
                        })
                    }
                }
                Err(TaskError::TimedOut {
                    timeout_seconds,
                    stdout: task_stdout,
                    stderr: task_stderr,
                }) => {
                    log.push_str(&task_stdout);
                    log.push_str(&task_stderr);
                    // Process group already killed by the runner; note cleanup for retry logs.
                    log.push_str("timeout cleanup: process group killed before retry\n");
                    stderr.push_str(&format!(
                        "[{}] task timed out after {timeout_seconds}s\n",
                        step.id
                    ));
                    Ok(LeafOutcome {
                        status: ExecutionStatus::Failed,
                        log,
                        stdout,
                        stderr,
                        proposal_ids: Vec::new(),
                    })
                }
                Err(err) => {
                    let msg = err.to_string();
                    log.push_str(&msg);
                    log.push('\n');
                    stderr.push_str(&format!("[{}] {msg}\n", step.id));
                    Ok(LeafOutcome {
                        status: ExecutionStatus::Failed,
                        log,
                        stdout,
                        stderr,
                        proposal_ids: Vec::new(),
                    })
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
                        resource: Some(workflow_rel.to_string()),
                        execution_id: Some(execution_id.to_string()),
                        step_id: Some(step.id.clone()),
                    },
                    summary: params.summary,
                    commands: params.commands,
                    affected_paths: params.affected_paths,
                    warnings: params.warnings,
                    created_at: String::new(),
                    status: Default::default(),
                    resolved_at: None,
                    applied_transaction_id: None,
                },
            )?;
            log.push_str(&format!("created proposal {}\n", created.id));
            stdout.push_str(&format!("[{}] proposal {}\n", step.id, created.id));
            Ok(LeafOutcome {
                status: ExecutionStatus::Succeeded,
                log,
                stdout,
                stderr,
                proposal_ids: vec![created.id],
            })
        }
        "notification" => {
            let params: NotificationParams = deserialize_with(&step.with, workflow_path, &step.id)?;
            let message = if params.message.is_empty() {
                format!("notification from step {}", step.id)
            } else {
                params.message
            };
            log.push_str(&message);
            log.push('\n');
            stdout.push_str(&format!("[{}] {message}\n", step.id));
            eprintln!("lattice workflow notification [{}]: {message}", step.id);
            Ok(LeafOutcome {
                status: ExecutionStatus::Succeeded,
                log,
                stdout,
                stderr,
                proposal_ids: Vec::new(),
            })
        }
        other => Err(WorkflowError::Invalid {
            path: workflow_path.to_path_buf(),
            message: format!("unknown step action {other:?}"),
        }),
    }
}

fn execute_leaf_with_retry(
    workspace_root: &Path,
    workflow_path: &Path,
    workflow_rel: &str,
    execution_id: &str,
    step: &WorkflowStep,
    runner: &TaskRunner,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<StepBatchOutcome> {
    let (max_attempts, backoff_seconds) = retry_budget(step.retry.as_ref());
    if let Some(rejected) =
        reject_unsafe_task_retry(workspace_root, workflow_path, step, max_attempts)?
    {
        return Ok(StepBatchOutcome {
            results: vec![WorkflowStepResult {
                id: step.id.clone(),
                action: step.action.clone(),
                status: ExecutionStatus::Failed,
                log: rejected.log,
                proposal_id: None,
                attempts: 1,
            }],
            status: ExecutionStatus::Failed,
            stdout: rejected.stdout,
            stderr: rejected.stderr,
            proposal_ids: Vec::new(),
        });
    }

    let (outcome, attempts_used, combined_log) =
        run_attempts(max_attempts, backoff_seconds, cancel, || {
            execute_leaf_once(
                workspace_root,
                workflow_path,
                workflow_rel,
                execution_id,
                step,
                runner,
            )
        })?;

    match outcome.status {
        ExecutionStatus::Succeeded => Ok(StepBatchOutcome {
            results: vec![WorkflowStepResult {
                id: step.id.clone(),
                action: step.action.clone(),
                status: ExecutionStatus::Succeeded,
                log: combined_log,
                proposal_id: primary_proposal_id(&outcome.proposal_ids),
                attempts: attempts_used,
            }],
            status: ExecutionStatus::Succeeded,
            stdout: outcome.stdout,
            stderr: String::new(),
            proposal_ids: outcome.proposal_ids,
        }),
        ExecutionStatus::Cancelled => Ok(StepBatchOutcome {
            results: vec![WorkflowStepResult {
                id: step.id.clone(),
                action: step.action.clone(),
                status: ExecutionStatus::Cancelled,
                log: combined_log,
                proposal_id: None,
                attempts: attempts_used.max(1),
            }],
            status: ExecutionStatus::Cancelled,
            stdout: String::new(),
            stderr: format!("[{}] cancelled\n", step.id),
            proposal_ids: Vec::new(),
        }),
        ExecutionStatus::Failed | ExecutionStatus::Running | ExecutionStatus::Abandoned => {
            Ok(StepBatchOutcome {
                results: vec![WorkflowStepResult {
                    id: step.id.clone(),
                    action: step.action.clone(),
                    status: ExecutionStatus::Failed,
                    log: combined_log,
                    proposal_id: None,
                    attempts: attempts_used,
                }],
                status: ExecutionStatus::Failed,
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                proposal_ids: Vec::new(),
            })
        }
    }
}

/// Retry loop shared by leaf steps (and unit-tested with a mock attempt fn).
fn run_attempts<F>(
    max_attempts: u32,
    backoff_seconds: u64,
    cancel: Option<&AtomicBool>,
    mut attempt_fn: F,
) -> WorkflowResult<(LeafOutcome, u32, String)>
where
    F: FnMut() -> WorkflowResult<LeafOutcome>,
{
    let max_attempts = max_attempts.max(1);
    let mut combined_log = String::new();
    let mut attempts_used = 0u32;
    let mut last = LeafOutcome {
        status: ExecutionStatus::Failed,
        log: String::new(),
        stdout: String::new(),
        stderr: String::new(),
        proposal_ids: Vec::new(),
    };

    for attempt in 1..=max_attempts {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            combined_log.push_str("cancelled before attempt\n");
            last.status = ExecutionStatus::Cancelled;
            return Ok((last, attempts_used.max(1), combined_log));
        }
        attempts_used = attempt;
        if max_attempts > 1 {
            combined_log.push_str(&format!("attempt {attempt}/{max_attempts}\n"));
        }
        last = attempt_fn()?;
        combined_log.push_str(&last.log);
        if last.status == ExecutionStatus::Succeeded {
            return Ok((last, attempts_used, combined_log));
        }
        if attempt < max_attempts {
            combined_log.push_str("retrying after failure\n");
            if interruptible_backoff(backoff_seconds, cancel) {
                combined_log.push_str("cancelled during backoff\n");
                last.status = ExecutionStatus::Cancelled;
                return Ok((last, attempts_used, combined_log));
            }
        }
    }

    Ok((last, attempts_used, combined_log))
}

fn execute_parallel_group(
    workspace_root: &Path,
    workflow_path: &Path,
    workflow_rel: &str,
    execution_id: &str,
    step: &WorkflowStep,
    runner: &TaskRunner,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<StepBatchOutcome> {
    let children = &step.parallel;
    let mut child_batches: Vec<StepBatchOutcome> = Vec::with_capacity(children.len());
    let mut cancelled = false;

    for chunk in children.chunks(MAX_PARALLEL_STEPS) {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            cancelled = true;
            break;
        }
        // Bounded fan-out: spawn up to MAX_PARALLEL_STEPS, join, then next wave.
        let wave: WorkflowResult<Vec<StepBatchOutcome>> = thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|child| {
                    scope.spawn(|| {
                        execute_leaf_with_retry(
                            workspace_root,
                            workflow_path,
                            workflow_rel,
                            execution_id,
                            child,
                            runner,
                            cancel,
                        )
                    })
                })
                .collect();
            let mut wave_out = Vec::with_capacity(handles.len());
            for handle in handles {
                match handle.join() {
                    Ok(Ok(batch)) => wave_out.push(batch),
                    Ok(Err(err)) => return Err(err),
                    Err(_) => {
                        return Err(WorkflowError::StepFailed {
                            step_id: step.id.clone(),
                            message: "parallel child thread panicked".into(),
                        });
                    }
                }
            }
            Ok(wave_out)
        });
        let wave = wave?;
        let wave_failed = wave
            .iter()
            .any(|batch| batch.status != ExecutionStatus::Succeeded);
        child_batches.extend(wave);
        if wave_failed {
            break;
        }
    }

    let mut results = Vec::new();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut proposal_ids = Vec::new();
    let mut any_failed = false;
    let mut any_cancelled = cancelled;

    for batch in child_batches {
        match batch.status {
            ExecutionStatus::Failed | ExecutionStatus::Abandoned => any_failed = true,
            ExecutionStatus::Cancelled => any_cancelled = true,
            ExecutionStatus::Succeeded | ExecutionStatus::Running => {}
        }
        stdout.push_str(&batch.stdout);
        stderr.push_str(&batch.stderr);
        extend_proposal_ids(&mut proposal_ids, batch.proposal_ids);
        results.extend(batch.results);
    }

    let group_status = if any_cancelled {
        ExecutionStatus::Cancelled
    } else if any_failed {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Succeeded
    };
    let child_count = step.parallel.len();
    let mut group_log = format!(
        "parallel: joined {joined}/{child_count} children (max concurrency {MAX_PARALLEL_STEPS})\n",
        joined = results.len(),
    );
    if any_failed {
        group_log.push_str("parallel: one or more children failed\n");
        if stderr.is_empty() {
            stderr.push_str(&format!("[{}] parallel group failed\n", step.id));
        }
    }
    if any_cancelled {
        group_log.push_str("parallel: cancelled\n");
        stderr.push_str(&format!("[{}] parallel group cancelled\n", step.id));
    }
    results.push(WorkflowStepResult {
        id: step.id.clone(),
        action: "parallel".into(),
        status: group_status,
        log: group_log,
        proposal_id: primary_proposal_id(&proposal_ids),
        attempts: 1,
    });
    if group_status == ExecutionStatus::Succeeded {
        stdout.push_str(&format!(
            "[{}] parallel ok ({child_count} children)\n",
            step.id
        ));
    }

    Ok(StepBatchOutcome {
        results,
        status: group_status,
        stdout,
        stderr,
        proposal_ids,
    })
}

fn execute_top_level_step(
    workspace_root: &Path,
    workflow_path: &Path,
    workflow_rel: &str,
    execution_id: &str,
    step: &WorkflowStep,
    runner: &TaskRunner,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<StepBatchOutcome> {
    if !step.parallel.is_empty() {
        return execute_parallel_group(
            workspace_root,
            workflow_path,
            workflow_rel,
            execution_id,
            step,
            runner,
            cancel,
        );
    }
    execute_leaf_with_retry(
        workspace_root,
        workflow_path,
        workflow_rel,
        execution_id,
        step,
        runner,
        cancel,
    )
}

/// Execute a workflow to completion (or until cancel / first failing step).
///
/// Steps run sequentially. A `parallel` group fans out its children concurrently
/// (bounded by [`MAX_PARALLEL_STEPS`]), joins, then the runner continues. Leaf
/// steps honor optional `retry` before the failure is treated as terminal.
///
/// When `execution_id` is `Some`, that id is used for the run record and for
/// `proposal.create` idempotency keys (`{execution_id}:{step_id}`). Desktop hosts
/// must pass the same id they expose to the UI.
pub fn run_workflow(
    workspace_root: &Path,
    workflow_path: &Path,
    manifest: &WorkflowManifest,
    trigger: &str,
    cancel: Option<&AtomicBool>,
    execution_id: Option<&str>,
) -> WorkflowResult<WorkflowRunRecord> {
    let execution_id = execution_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let started_at = now_iso();
    let workflow_rel = workflow_path
        .strip_prefix(workspace_root)
        .unwrap_or(workflow_path)
        .to_string_lossy()
        .replace('\\', "/");

    let placeholder = WorkflowRunRecord {
        workflow_path: workflow_rel.clone(),
        trigger: trigger.to_string(),
        execution: ExecutionResult {
            id: execution_id.clone(),
            status: ExecutionStatus::Running,
            stdout: String::new(),
            stderr: String::new(),
            started_at: started_at.clone(),
            finished_at: None,
            outputs: Vec::new(),
            proposal_id: None,
            proposal_ids: Vec::new(),
        },
        steps: Vec::new(),
    };
    save_workflow_run(workspace_root, &placeholder)?;

    run_workflow_body(
        workspace_root,
        workflow_path,
        manifest,
        trigger,
        cancel,
        execution_id,
        started_at,
        workflow_rel,
    )
}

/// Like [`run_workflow`], but accepts an owned optional execution id (daemon schedule path).
pub fn run_workflow_with_id(
    workspace_root: &Path,
    workflow_path: &Path,
    manifest: &WorkflowManifest,
    trigger: &str,
    execution_id: Option<String>,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<WorkflowRunRecord> {
    run_workflow(
        workspace_root,
        workflow_path,
        manifest,
        trigger,
        cancel,
        execution_id.as_deref(),
    )
}

fn run_workflow_body(
    workspace_root: &Path,
    workflow_path: &Path,
    manifest: &WorkflowManifest,
    trigger: &str,
    cancel: Option<&AtomicBool>,
    execution_id: String,
    started_at: String,
    workflow_rel: String,
) -> WorkflowResult<WorkflowRunRecord> {
    let mut step_results = Vec::new();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut proposal_ids = Vec::new();
    let mut status = ExecutionStatus::Succeeded;

    let runner = TaskRunner::new();

    for step in &manifest.steps {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            status = ExecutionStatus::Cancelled;
            stderr.push_str("workflow cancelled\n");
            break;
        }

        let batch = execute_top_level_step(
            workspace_root,
            workflow_path,
            &workflow_rel,
            &execution_id,
            step,
            &runner,
            cancel,
        )?;
        stdout.push_str(&batch.stdout);
        stderr.push_str(&batch.stderr);
        extend_proposal_ids(&mut proposal_ids, batch.proposal_ids);
        step_results.extend(batch.results);

        match batch.status {
            ExecutionStatus::Succeeded => {}
            ExecutionStatus::Failed | ExecutionStatus::Abandoned => {
                status = ExecutionStatus::Failed;
                break;
            }
            ExecutionStatus::Cancelled => {
                status = ExecutionStatus::Cancelled;
                break;
            }
            ExecutionStatus::Running => {
                // Steps report terminal statuses only; treat unexpected as failure.
                status = ExecutionStatus::Failed;
                break;
            }
        }
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
            proposal_id: primary_proposal_id(&proposal_ids),
            proposal_ids,
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
    execution_id: Option<&str>,
) -> WorkflowResult<WorkflowRunRecord> {
    let manifest = WorkflowManifest::load(workflow_path)?;
    let trigger = trigger_override.unwrap_or_else(|| trigger_label(&manifest.trigger));
    run_workflow(
        workspace_root,
        workflow_path,
        &manifest,
        trigger,
        cancel,
        execution_id,
    )
}

/// Like [`load_and_run_workflow`], with an optional owned pre-assigned execution id.
pub fn load_and_run_workflow_with_id(
    workspace_root: &Path,
    workflow_path: &Path,
    trigger_override: Option<&str>,
    execution_id: Option<String>,
    cancel: Option<&AtomicBool>,
) -> WorkflowResult<WorkflowRunRecord> {
    load_and_run_workflow(
        workspace_root,
        workflow_path,
        trigger_override,
        cancel,
        execution_id.as_deref(),
    )
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
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Succeeded);
        let proposal_id = record.execution.proposal_id.expect("proposal id");
        assert_eq!(record.execution.proposal_ids, vec![proposal_id.clone()]);
        let proposal = crate::load_proposal(dir.path(), &proposal_id).unwrap();
        assert_eq!(proposal.source.source_type, ProposalSourceType::Workflow);
        assert_eq!(
            proposal.source.resource.as_deref(),
            Some("Simple.workflow.yaml")
        );
        assert_eq!(
            proposal.source.execution_id.as_deref(),
            Some(record.execution.id.as_str())
        );
        assert_eq!(proposal.source.step_id.as_deref(), Some("propose"));
        assert_eq!(record.steps.len(), 2);
        assert!(workflow_runs_dir(dir.path())
            .join(format!("{}.json", record.execution.id))
            .is_file());
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

    #[test]
    fn parses_schedule_interval_trigger() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Hourly
trigger:
  type: schedule
  interval_seconds: 3600
steps: []
"#;
        let manifest =
            WorkflowManifest::parse(Path::new("Hourly.workflow.yaml"), yaml).expect("parse");
        assert_eq!(
            manifest.trigger,
            WorkflowTrigger::Schedule(ScheduleTrigger {
                interval_seconds: Some(3600),
                cron: None,
                timezone: None,
            })
        );
        assert!(!manifest.matches_resource_change("Notes/A.md"));
        assert!(!manifest.matches_form_submitted("CRM.data", "Intake", None));
    }

    #[test]
    fn parses_schedule_cron_trigger() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Nightly
trigger:
  type: schedule
  cron: "0 2 * * *"
  timezone: America/Los_Angeles
steps: []
"#;
        let manifest =
            WorkflowManifest::parse(Path::new("Nightly.workflow.yaml"), yaml).expect("parse");
        match &manifest.trigger {
            WorkflowTrigger::Schedule(schedule) => {
                assert!(schedule.interval_seconds.is_none());
                assert_eq!(schedule.cron.as_deref(), Some("0 2 * * *"));
                assert_eq!(schedule.timezone.as_deref(), Some("America/Los_Angeles"));
            }
            other => panic!("expected schedule trigger, got {other:?}"),
        }
    }

    #[test]
    fn parses_scheduled_fixture_round_trip() {
        let path = fixture("Scheduled.workflow.yaml");
        let manifest = WorkflowManifest::load(&path).expect("load");
        assert!(matches!(
            &manifest.trigger,
            WorkflowTrigger::Schedule(ScheduleTrigger {
                interval_seconds: Some(3600),
                cron: None,
                timezone: None,
            })
        ));
        let rewritten = serde_yaml::to_string(&manifest).expect("serialize");
        let again = WorkflowManifest::parse(&path, &rewritten).expect("reparse");
        assert_eq!(again, manifest);
    }

    #[test]
    fn rejects_schedule_without_interval_or_cron() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: EmptySchedule
trigger:
  type: schedule
steps: []
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(
            err.to_string().contains("interval_seconds") && err.to_string().contains("cron"),
            "{err}"
        );
    }

    #[test]
    fn rejects_schedule_zero_interval() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Zero
trigger:
  type: schedule
  interval_seconds: 0
steps: []
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(err.to_string().contains("interval_seconds"), "{err}");
    }

    #[test]
    fn rejects_schedule_unknown_fields() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Extra
trigger:
  type: schedule
  interval_seconds: 60
  every: hour
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
    fn rejects_schedule_invalid_cron_field_count() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: BadCron
trigger:
  type: schedule
  cron: "0 * *"
steps: []
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(err.to_string().contains("cron"), "{err}");
    }

    #[test]
    fn parses_retry_and_parallel_fixture() {
        let path = fixture("RetryParallel.workflow.yaml");
        let manifest = WorkflowManifest::load(&path).expect("load");
        assert_eq!(manifest.steps.len(), 2);
        let retry = manifest.steps[0].retry.as_ref().expect("retry");
        assert_eq!(retry.max_attempts, 3);
        assert_eq!(retry.backoff_seconds, 0);
        assert_eq!(manifest.steps[1].parallel.len(), 2);
        assert_eq!(manifest.steps[1].action, "parallel");
    }

    #[test]
    fn rejects_retry_max_attempts_zero() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: BadRetry
trigger:
  type: manual
steps:
  - id: n
    action: notification
    retry:
      max_attempts: 0
    with:
      message: x
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(err.to_string().contains("max_attempts"), "{err}");
    }

    #[test]
    fn rejects_nested_parallel() {
        let yaml = r#"
format: lattice-workflow
version: 1
name: Nested
trigger:
  type: manual
steps:
  - id: outer
    parallel:
      - id: inner
        parallel:
          - id: leaf
            action: notification
            with:
              message: x
"#;
        let err = WorkflowManifest::parse(Path::new("bad.workflow.yaml"), yaml).unwrap_err();
        assert!(err.to_string().contains("nested parallel"), "{err}");
    }

    #[test]
    fn run_attempts_retries_then_succeeds() {
        let mut calls = 0u32;
        let (outcome, attempts, log) = run_attempts(3, 0, None, || {
            calls += 1;
            if calls < 3 {
                Ok(LeafOutcome {
                    status: ExecutionStatus::Failed,
                    log: format!("fail-{calls}\n"),
                    stdout: String::new(),
                    stderr: format!("err-{calls}\n"),
                    proposal_ids: Vec::new(),
                })
            } else {
                Ok(LeafOutcome {
                    status: ExecutionStatus::Succeeded,
                    log: "ok\n".into(),
                    stdout: "ok\n".into(),
                    stderr: String::new(),
                    proposal_ids: Vec::new(),
                })
            }
        })
        .expect("retry loop");
        assert_eq!(outcome.status, ExecutionStatus::Succeeded);
        assert_eq!(attempts, 3);
        assert_eq!(calls, 3);
        assert!(log.contains("attempt 1/3"));
        assert!(log.contains("retrying after failure"));
        assert!(log.contains("ok"));
    }

    #[test]
    fn run_attempts_retries_then_fails() {
        let mut calls = 0u32;
        let (outcome, attempts, log) = run_attempts(2, 0, None, || {
            calls += 1;
            Ok(LeafOutcome {
                status: ExecutionStatus::Failed,
                log: format!("fail-{calls}\n"),
                stdout: String::new(),
                stderr: format!("err-{calls}\n"),
                proposal_ids: Vec::new(),
            })
        })
        .expect("retry loop");
        assert_eq!(outcome.status, ExecutionStatus::Failed);
        assert_eq!(attempts, 2);
        assert_eq!(calls, 2);
        assert!(log.contains("attempt 2/2"));
    }

    #[test]
    fn run_retries_exhausted_on_missing_task() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("RetryFail.workflow.yaml");
        let yaml = r#"
format: lattice-workflow
version: 1
name: Retry fail
trigger:
  type: manual
steps:
  - id: missing
    action: task.run
    retry:
      max_attempts: 3
      backoff_seconds: 0
    with:
      task: DoesNotExist.task
"#;
        fs::write(&workflow_path, yaml).unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Failed);
        assert_eq!(record.steps.len(), 1);
        assert_eq!(record.steps[0].attempts, 3);
        assert_eq!(record.steps[0].status, ExecutionStatus::Failed);
        assert!(record.steps[0].log.contains("attempt 1/3"));
        assert!(record.steps[0].log.contains("retrying after failure"));
    }

    #[test]
    fn run_parallel_group_joins_before_next_step() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("Parallel.workflow.yaml");
        let yaml = r#"
format: lattice-workflow
version: 1
name: Parallel join
trigger:
  type: manual
steps:
  - id: fan
    action: parallel
    parallel:
      - id: left
        action: notification
        with:
          message: left-done
      - id: right
        action: notification
        with:
          message: right-done
  - id: after
    action: notification
    with:
      message: after-join
"#;
        fs::write(&workflow_path, yaml).unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Succeeded);
        let ids: Vec<_> = record.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["left", "right", "fan", "after"]);
        assert_eq!(record.steps[2].action, "parallel");
        assert_eq!(record.steps[2].status, ExecutionStatus::Succeeded);
        assert!(record.execution.stdout.contains("after-join"));
        assert!(record.steps[2].log.contains("joined 2/2"));
    }

    #[test]
    fn run_parallel_failure_stops_workflow() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("ParallelFail.workflow.yaml");
        let yaml = r#"
format: lattice-workflow
version: 1
name: Parallel fail
trigger:
  type: manual
steps:
  - id: fan
    parallel:
      - id: ok
        action: notification
        with:
          message: ok
      - id: bad
        action: task.run
        with:
          task: Missing.task
  - id: after
    action: notification
    with:
      message: should-not-run
"#;
        fs::write(&workflow_path, yaml).unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Failed);
        assert!(record
            .steps
            .iter()
            .any(|s| s.id == "fan" && s.status == ExecutionStatus::Failed));
        assert!(record.steps.iter().all(|s| s.id != "after"));
        assert!(!record.execution.stdout.contains("should-not-run"));
    }

    #[test]
    fn run_parallel_respects_max_concurrency_waves() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("ParallelWaves.workflow.yaml");
        // One more than MAX_PARALLEL_STEPS forces a second wave; all must still join.
        let mut children = String::new();
        for index in 0..=MAX_PARALLEL_STEPS {
            children.push_str(&format!(
                r#"
      - id: c{index}
        action: notification
        with:
          message: child-{index}
"#
            ));
        }
        let yaml = format!(
            r#"
format: lattice-workflow
version: 1
name: Waves
trigger:
  type: manual
steps:
  - id: fan
    parallel:
{children}
"#
        );
        fs::write(&workflow_path, yaml).unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Succeeded);
        let child_results = record.steps.len() - 1; // exclude group summary
        assert_eq!(child_results, MAX_PARALLEL_STEPS + 1);
        let fan = record.steps.last().expect("fan");
        assert_eq!(fan.id, "fan");
        assert!(fan.log.contains(&format!(
            "joined {}/{}",
            MAX_PARALLEL_STEPS + 1,
            MAX_PARALLEL_STEPS + 1
        )));
    }

    #[test]
    fn discover_scheduled_workflows_skips_disabled_and_non_schedule() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(
            root.join("On.workflow.yaml"),
            r#"
format: lattice-workflow
version: 1
name: On
enabled: true
trigger:
  type: schedule
  interval_seconds: 60
steps: []
"#,
        )
        .unwrap();
        fs::write(
            root.join("Off.workflow.yaml"),
            r#"
format: lattice-workflow
version: 1
name: Off
enabled: false
trigger:
  type: schedule
  interval_seconds: 60
steps: []
"#,
        )
        .unwrap();
        fs::write(
            root.join("Manual.workflow.yaml"),
            r#"
format: lattice-workflow
version: 1
name: Manual
trigger:
  type: manual
steps: []
"#,
        )
        .unwrap();

        let found = discover_scheduled_workflows(root).expect("discover");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.name, "On");
        assert_eq!(found[0].schedule().interval_seconds, Some(60));
    }

    #[test]
    fn evaluate_schedule_due_interval_and_cron_deferred() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let interval = ScheduleTrigger {
            interval_seconds: Some(60),
            cron: Some("0 * * * *".into()),
            timezone: None,
        };
        assert_eq!(
            evaluate_schedule_due(&interval, None, now),
            ScheduleDue::Due
        );
        assert_eq!(
            evaluate_schedule_due(&interval, Some(now - Duration::from_secs(59)), now),
            ScheduleDue::NotDue
        );
        assert_eq!(
            evaluate_schedule_due(&interval, Some(now - Duration::from_secs(60)), now),
            ScheduleDue::Due
        );

        let cron_only = ScheduleTrigger {
            interval_seconds: None,
            cron: Some("0 2 * * *".into()),
            timezone: Some("UTC".into()),
        };
        assert_eq!(
            evaluate_schedule_due(&cron_only, None, now),
            ScheduleDue::CronDeferred
        );
    }

    #[test]
    fn last_schedule_run_at_reads_persisted_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let workflow = root.join("Tick.workflow.yaml");
        fs::write(
            &workflow,
            r#"
format: lattice-workflow
version: 1
name: Tick
trigger:
  type: schedule
  interval_seconds: 1
steps:
  - id: note
    action: notification
    with:
      message: tick
"#,
        )
        .unwrap();
        let record =
            load_and_run_workflow(root, &workflow, Some("schedule"), None, None).expect("run");
        assert_eq!(record.trigger, "schedule");
        let last = last_schedule_run_at(root, "Tick.workflow.yaml")
            .expect("history")
            .expect("started_at");
        let parsed = parse_iso8601_z(&record.execution.started_at).expect("parse");
        assert_eq!(last, parsed);
    }

    #[test]
    fn proposal_create_retry_dedupes_existing_pending() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("Dedupe.workflow.yaml");
        let yaml = r##"
format: lattice-workflow
version: 1
name: Dedupe
trigger:
  type: manual
steps:
  - id: propose
    action: proposal.create
    retry:
      max_attempts: 2
      backoff_seconds: 0
    with:
      summary: Create once
      commands:
        - type: page-create
          path: Notes/Once.md
          content: "# Once\n"
"##;
        fs::write(&workflow_path, yaml).unwrap();
        let execution_id = "exec-stable-dedupe";
        let record = run_workflow(
            dir.path(),
            &workflow_path,
            &WorkflowManifest::load(&workflow_path).unwrap(),
            "manual",
            None,
            Some(execution_id),
        )
        .expect("run");
        assert_eq!(record.execution.id, execution_id);
        assert_eq!(record.execution.status, ExecutionStatus::Succeeded);
        assert_eq!(record.execution.proposal_ids.len(), 1);
        let first_id = record.execution.proposal_ids[0].clone();

        // Simulate post-persist failure: create again with the same key.
        let again = create_proposal(
            dir.path(),
            TransactionProposal {
                id: String::new(),
                source: ProposalSource {
                    source_type: ProposalSourceType::Workflow,
                    resource: Some("Dedupe.workflow.yaml".into()),
                    execution_id: Some(execution_id.into()),
                    step_id: Some("propose".into()),
                },
                summary: "Create once again".into(),
                commands: vec![],
                affected_paths: vec![],
                warnings: vec![],
                created_at: String::new(),
                status: Default::default(),
                resolved_at: None,
                applied_transaction_id: None,
            },
        )
        .unwrap();
        assert_eq!(again.id, first_id);
        assert_eq!(crate::list_proposal_summaries(dir.path()).unwrap().len(), 1);
    }

    #[test]
    fn rejects_retry_on_non_idempotent_task_without_unsafe_ack() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let task_dir = dir.path().join("Unsafe.task");
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("task.yaml"),
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
entrypoint:
  command: [python, main.py]
"#,
        )
        .unwrap();
        fs::write(task_dir.join("main.py"), "print('ok')\n").unwrap();
        let workflow_path = dir.path().join("UnsafeRetry.workflow.yaml");
        fs::write(
            &workflow_path,
            r#"
format: lattice-workflow
version: 1
name: Unsafe retry
trigger:
  type: manual
steps:
  - id: run
    action: task.run
    retry:
      max_attempts: 3
      backoff_seconds: 0
    with:
      task: Unsafe.task
"#,
        )
        .unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Failed);
        assert_eq!(record.steps[0].attempts, 1);
        assert!(record.steps[0].log.contains("allow_unsafe_retry"));
        assert!(record
            .execution
            .stderr
            .contains("not marked execution.idempotent"));
    }

    #[test]
    fn allows_retry_when_task_declares_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let task_dir = dir.path().join("Safe.task");
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("task.yaml"),
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
entrypoint:
  command: [python, main.py]
execution:
  idempotent: true
"#,
        )
        .unwrap();
        // Entrypoint that does not need uv project markers beyond what runner expects —
        // missing uv will fail attempts; we only assert the unsafe gate did not fire.
        fs::write(task_dir.join("main.py"), "print('ok')\n").unwrap();
        let workflow_path = dir.path().join("SafeRetry.workflow.yaml");
        fs::write(
            &workflow_path,
            r#"
format: lattice-workflow
version: 1
name: Safe retry
trigger:
  type: manual
steps:
  - id: run
    action: task.run
    retry:
      max_attempts: 2
      backoff_seconds: 0
    with:
      task: Safe.task
"#,
        )
        .unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert!(
            !record.steps[0].log.contains("allow_unsafe_retry"),
            "idempotent task should not hit unsafe rejection: {}",
            record.steps[0].log
        );
        // Either succeeds (uv present) or retries then fails (uv missing) — not single-attempt reject.
        assert!(record.steps[0].attempts >= 1);
        if record.execution.status == ExecutionStatus::Failed {
            assert!(
                record.steps[0].attempts > 1
                    || record.steps[0].log.contains("attempt 1/2")
                    || record.execution.stderr.contains("uv")
                    || record.execution.stderr.contains("MissingTool")
                    || record.steps[0].log.contains("missing"),
                "unexpected failure without retry: {}",
                record.steps[0].log
            );
        }
    }

    #[test]
    fn run_attempts_cancels_during_backoff() {
        use std::sync::atomic::AtomicU32;
        use std::sync::Arc;

        let cancel = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicU32::new(0));
        let cancel_thread = Arc::clone(&cancel);
        let calls_thread = Arc::clone(&calls);
        let handle = thread::spawn(move || {
            run_attempts(3, 2, Some(cancel_thread.as_ref()), || {
                let n = calls_thread.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(LeafOutcome {
                    status: ExecutionStatus::Failed,
                    log: format!("fail-{n}\n"),
                    stdout: String::new(),
                    stderr: format!("err-{n}\n"),
                    proposal_ids: Vec::new(),
                })
            })
        });
        // Flip cancel shortly after the first failure enters backoff.
        thread::sleep(Duration::from_millis(100));
        cancel.store(true, Ordering::SeqCst);
        let (outcome, attempts, log) = handle.join().unwrap().expect("retry loop");
        assert_eq!(outcome.status, ExecutionStatus::Cancelled);
        assert_eq!(attempts, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(log.contains("cancelled during backoff"));
    }

    #[test]
    fn parallel_proposal_children_retain_all_ids() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("ParallelProposals.workflow.yaml");
        fs::write(
            &workflow_path,
            r##"
format: lattice-workflow
version: 1
name: Parallel proposals
trigger:
  type: manual
steps:
  - id: fan
    action: parallel
    parallel:
      - id: left
        action: proposal.create
        with:
          summary: Left
          commands:
            - type: page-create
              path: Notes/Left.md
              content: "# Left\n"
      - id: right
        action: proposal.create
        with:
          summary: Right
          commands:
            - type: page-create
              path: Notes/Right.md
              content: "# Right\n"
"##,
        )
        .unwrap();
        let record = load_and_run_workflow(dir.path(), &workflow_path, Some("manual"), None, None)
            .expect("run");
        assert_eq!(record.execution.status, ExecutionStatus::Succeeded);
        assert_eq!(record.execution.proposal_ids.len(), 2);
        let left = record
            .steps
            .iter()
            .find(|s| s.id == "left")
            .and_then(|s| s.proposal_id.clone())
            .expect("left proposal");
        let right = record
            .steps
            .iter()
            .find(|s| s.id == "right")
            .and_then(|s| s.proposal_id.clone())
            .expect("right proposal");
        assert_ne!(left, right);
        assert!(record.execution.proposal_ids.contains(&left));
        assert!(record.execution.proposal_ids.contains(&right));
        assert_eq!(
            record.execution.proposal_id.as_deref(),
            Some(record.execution.proposal_ids[0].as_str())
        );
    }

    #[test]
    fn run_workflow_honors_stable_execution_id() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let workflow_path = dir.path().join("StableId.workflow.yaml");
        fs::write(
            &workflow_path,
            r#"
format: lattice-workflow
version: 1
name: Stable
trigger:
  type: manual
steps:
  - id: note
    action: notification
    with:
      message: hi
"#,
        )
        .unwrap();
        let record = run_workflow(
            dir.path(),
            &workflow_path,
            &WorkflowManifest::load(&workflow_path).unwrap(),
            "manual",
            None,
            Some("desktop-stable-id"),
        )
        .expect("run");
        assert_eq!(record.execution.id, "desktop-stable-id");
        assert!(workflow_runs_dir(dir.path())
            .join("desktop-stable-id.json")
            .is_file());
    }

    #[test]
    fn reconcile_abandoned_marks_stranded_running_runs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lattice.yaml"), "id: test\ntitle: Test\n").unwrap();
        let running = WorkflowRunRecord {
            workflow_path: "Stuck.workflow.yaml".into(),
            trigger: "schedule".into(),
            execution: ExecutionResult {
                id: "exec-abandoned-1".into(),
                status: ExecutionStatus::Running,
                stdout: String::new(),
                stderr: String::new(),
                started_at: "2026-07-24T12:00:00Z".into(),
                finished_at: None,
                outputs: Vec::new(),
                proposal_id: None,
                proposal_ids: Vec::new(),
            },
            steps: Vec::new(),
        };
        save_workflow_run(dir.path(), &running).expect("save running");
        let marked = reconcile_abandoned_workflow_runs(dir.path()).expect("reconcile");
        assert_eq!(marked, 1);
        let loaded = load_workflow_run(dir.path(), "exec-abandoned-1").expect("load");
        assert_eq!(loaded.execution.status, ExecutionStatus::Abandoned);
        assert!(loaded.execution.finished_at.is_some());
        assert!(loaded.execution.stderr.contains("abandoned"));
        assert_eq!(reconcile_abandoned_workflow_runs(dir.path()).unwrap(), 0);
    }
}
