//! Tauri-side wiring for `*.workflow.yaml` (load / run / cancel / enable).
//!
//! Keeps `lattice-commands` free of Tauri: this module owns execution state,
//! workspace containment, background spawn, debounce for resource.changed,
//! and event emission. Form submissions are hooked from `data::insert_record`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lattice_commands::{
    discover_workflows, list_workflow_runs, run_workflow, save_workflow_run, set_workflow_enabled,
    ExecutionResult, ExecutionStatus, WorkflowManifest, WorkflowRunRecord, WorkflowTrigger,
};

/// One in-flight workflow surfaced in the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningWorkflow {
    pub execution_id: String,
    pub workflow_path: String,
    pub label: String,
    pub started_at: String,
}
use lattice_core::Workspace;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

const WORKFLOW_EXECUTION_EVENT: &str = "workflow-execution-updated";
const RESOURCE_CHANGE_DEBOUNCE: Duration = Duration::from_millis(400);

/// Tauri-managed workflow execution + debounce state for the open workspace.
#[derive(Default)]
pub struct WorkflowState {
    executions: Mutex<HashMap<String, LiveExecution>>,
    /// Workspace root currently watched for automatic triggers.
    root: Mutex<Option<PathBuf>>,
    /// Debounced resource.changed keys → last fire time.
    debounce: Mutex<HashMap<String, Instant>>,
}

struct LiveExecution {
    result: Arc<Mutex<WorkflowRunRecord>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLoadRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunRequest {
    pub root: String,
    pub rel_path: String,
    #[serde(default)]
    pub execution_id: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunResponse {
    pub execution_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowExecutionRequest {
    pub execution_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSetEnabledRequest {
    pub root: String,
    pub rel_path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowListRunsRequest {
    pub root: String,
    pub rel_path: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTriggerView {
    #[serde(rename = "type")]
    pub trigger_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepView {
    pub id: String,
    pub action: String,
    pub with: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestView {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub enabled: bool,
    pub trigger: WorkflowTriggerView,
    pub steps: Vec<WorkflowStepView>,
    pub raw_yaml: String,
}

fn open_workspace(root: &Path) -> Result<Workspace, String> {
    Workspace::open(root).map_err(|err| err.to_string())
}

fn resolve_workflow(workspace: &Workspace, rel_path: &str) -> Result<PathBuf, String> {
    let file = workspace.root().join(rel_path);
    if !file.is_file() {
        return Err(format!("workflow not found: {rel_path}"));
    }
    let canonical_root = workspace
        .root()
        .canonicalize()
        .map_err(|err| err.to_string())?;
    let canonical = file.canonicalize().map_err(|err| err.to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("workflow path escapes workspace root".into());
    }
    Ok(canonical)
}

fn trigger_view(trigger: &WorkflowTrigger) -> WorkflowTriggerView {
    match trigger {
        WorkflowTrigger::Manual => WorkflowTriggerView {
            trigger_type: "manual".into(),
            paths: None,
            form: None,
            package: None,
            form_id: None,
            interval_seconds: None,
            cron: None,
            timezone: None,
        },
        WorkflowTrigger::ResourceChanged { paths } => WorkflowTriggerView {
            trigger_type: "resource.changed".into(),
            paths: Some(paths.clone()),
            form: None,
            package: None,
            form_id: None,
            interval_seconds: None,
            cron: None,
            timezone: None,
        },
        WorkflowTrigger::FormSubmitted {
            form,
            package,
            form_id,
        } => WorkflowTriggerView {
            trigger_type: "form.submitted".into(),
            paths: None,
            form: form.clone(),
            package: package.clone(),
            form_id: form_id.clone(),
            interval_seconds: None,
            cron: None,
            timezone: None,
        },
        WorkflowTrigger::Schedule(schedule) => WorkflowTriggerView {
            trigger_type: "schedule".into(),
            paths: None,
            form: None,
            package: None,
            form_id: None,
            interval_seconds: schedule.interval_seconds,
            cron: schedule.cron.clone(),
            timezone: schedule.timezone.clone(),
        },
    }
}

fn manifest_view(manifest: WorkflowManifest, raw_yaml: String) -> Result<WorkflowManifestView, String> {
    let mut steps = Vec::new();
    for step in &manifest.steps {
        let with = serde_json::to_value(&step.with).map_err(|err| err.to_string())?;
        steps.push(WorkflowStepView {
            id: step.id.clone(),
            action: step.action.clone(),
            with,
        });
    }
    Ok(WorkflowManifestView {
        format: manifest.format,
        version: manifest.version,
        name: manifest.name,
        enabled: manifest.enabled,
        trigger: trigger_view(&manifest.trigger),
        steps,
        raw_yaml,
    })
}

fn update_and_emit(app: &AppHandle, record: &Arc<Mutex<WorkflowRunRecord>>) {
    let snapshot = match record.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };
    if let Err(err) = app.emit(WORKFLOW_EXECUTION_EVENT, &snapshot) {
        eprintln!("lattice: failed to emit {WORKFLOW_EXECUTION_EVENT}: {err}");
    }
    crate::tray::refresh_from_workflows(app);
}

fn patch_record(
    record: &Arc<Mutex<WorkflowRunRecord>>,
    patch: impl FnOnce(&mut WorkflowRunRecord),
) {
    if let Ok(mut guard) = record.lock() {
        patch(&mut guard);
    }
}

fn now_iso() -> String {
    lattice_commands::proposal_now_iso()
}

/// Remember the active workspace root so automatic triggers know where to scan.
pub fn set_active_workspace(state: &WorkflowState, root: Option<PathBuf>) {
    if let Ok(mut slot) = state.root.lock() {
        *slot = root;
    }
}

/// Workspace root last used for workflow execution (for tray open-resource).
pub fn active_workspace_root(state: &WorkflowState) -> Option<PathBuf> {
    state.root.lock().ok().and_then(|slot| slot.clone())
}

/// Running workflow executions, newest first.
pub fn running_workflows(state: &WorkflowState) -> Vec<RunningWorkflow> {
    let Ok(map) = state.executions.lock() else {
        return Vec::new();
    };
    let mut running = map
        .iter()
        .filter_map(|(execution_id, live)| {
            let record = live.result.lock().ok()?;
            if record.execution.status != ExecutionStatus::Running {
                return None;
            }
            Some(RunningWorkflow {
                execution_id: execution_id.clone(),
                workflow_path: record.workflow_path.clone(),
                label: workflow_tray_label(&record.workflow_path),
                started_at: record.execution.started_at.clone(),
            })
        })
        .collect::<Vec<_>>();
    running.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    running
}

fn workflow_tray_label(workflow_path: &str) -> String {
    let file_name = Path::new(workflow_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(workflow_path);
    file_name
        .strip_suffix(".workflow.yaml")
        .or_else(|| file_name.strip_suffix(".workflow.yml"))
        .unwrap_or(file_name)
        .to_string()
}

/// Load and validate a workflow YAML for the desktop surface.
#[tauri::command]
pub fn workflow_load(request: WorkflowLoadRequest) -> Result<WorkflowManifestView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let path = resolve_workflow(&workspace, &request.rel_path)?;
    let raw_yaml = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let manifest = WorkflowManifest::parse(&path, &raw_yaml).map_err(|err| err.to_string())?;
    manifest_view(manifest, raw_yaml)
}

/// Start a background workflow run; returns immediately with an execution id.
#[tauri::command]
pub fn workflow_run(
    request: WorkflowRunRequest,
    app: AppHandle,
    state: tauri::State<'_, WorkflowState>,
) -> Result<WorkflowRunResponse, String> {
    spawn_workflow_run(&app, &state, request)
}

fn spawn_workflow_run(
    app: &AppHandle,
    state: &WorkflowState,
    request: WorkflowRunRequest,
) -> Result<WorkflowRunResponse, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let path = resolve_workflow(&workspace, &request.rel_path)?;
    set_active_workspace(state, Some(workspace.root().to_path_buf()));

    let execution_id = request
        .execution_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let trigger = request.trigger.unwrap_or_else(|| "manual".into());

    let placeholder = WorkflowRunRecord {
        workflow_path: request.rel_path.clone(),
        trigger: trigger.clone(),
        execution: ExecutionResult {
            id: execution_id.clone(),
            status: ExecutionStatus::Running,
            stdout: String::new(),
            stderr: String::new(),
            started_at: now_iso(),
            finished_at: None,
            outputs: Vec::new(),
            proposal_id: None,
        },
        steps: Vec::new(),
    };
    let record = Arc::new(Mutex::new(placeholder));
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut map = state
            .executions
            .lock()
            .map_err(|_| "workflow state lock poisoned")?;
        map.insert(
            execution_id.clone(),
            LiveExecution {
                result: Arc::clone(&record),
                cancel: Arc::clone(&cancel),
            },
        );
    }

    update_and_emit(app, &record);

    let app_thread = app.clone();
    let record_thread = Arc::clone(&record);
    let cancel_thread = Arc::clone(&cancel);
    let root = workspace.root().to_path_buf();
    let workflow_path = path;

    thread::spawn(move || {
        let manifest = match WorkflowManifest::load(&workflow_path) {
            Ok(manifest) => manifest,
            Err(err) => {
                patch_record(&record_thread, |r| {
                    r.execution.status = ExecutionStatus::Failed;
                    r.execution.stderr = err.to_string();
                    r.execution.finished_at = Some(now_iso());
                });
                if let Ok(guard) = record_thread.lock() {
                    let _ = save_workflow_run(&root, &guard);
                }
                update_and_emit(&app_thread, &record_thread);
                return;
            }
        };

        match run_workflow(
            &root,
            &workflow_path,
            &manifest,
            &trigger,
            Some(&cancel_thread),
        ) {
            Ok(finished) => {
                patch_record(&record_thread, |r| *r = finished);
            }
            Err(err) => {
                patch_record(&record_thread, |r| {
                    if cancel_thread.load(Ordering::SeqCst) {
                        r.execution.status = ExecutionStatus::Cancelled;
                    } else {
                        r.execution.status = ExecutionStatus::Failed;
                    }
                    r.execution.stderr = err.to_string();
                    r.execution.finished_at = Some(now_iso());
                });
                if let Ok(guard) = record_thread.lock() {
                    let _ = save_workflow_run(&root, &guard);
                }
            }
        }
        update_and_emit(&app_thread, &record_thread);
    });

    Ok(WorkflowRunResponse { execution_id })
}

/// Request cancellation between steps (best-effort).
#[tauri::command]
pub fn workflow_cancel(
    request: WorkflowExecutionRequest,
    state: tauri::State<'_, WorkflowState>,
) -> Result<(), String> {
    let map = state
        .executions
        .lock()
        .map_err(|_| "workflow state lock poisoned")?;
    let live = map
        .get(&request.execution_id)
        .ok_or_else(|| format!("unknown workflow execution: {}", request.execution_id))?;
    live.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// Poll the current run record for an execution id.
#[tauri::command]
pub fn workflow_execution_status(
    request: WorkflowExecutionRequest,
    state: tauri::State<'_, WorkflowState>,
) -> Result<WorkflowRunRecord, String> {
    let map = state
        .executions
        .lock()
        .map_err(|_| "workflow state lock poisoned")?;
    let live = map
        .get(&request.execution_id)
        .ok_or_else(|| format!("unknown workflow execution: {}", request.execution_id))?;
    live.result
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "workflow result lock poisoned".into())
}

/// Persist `enabled` on the workflow YAML.
#[tauri::command]
pub fn workflow_set_enabled(
    request: WorkflowSetEnabledRequest,
) -> Result<WorkflowManifestView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let path = resolve_workflow(&workspace, &request.rel_path)?;
    let manifest =
        set_workflow_enabled(&path, request.enabled).map_err(|err| err.to_string())?;
    let raw_yaml = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    manifest_view(manifest, raw_yaml)
}

/// Recent run history for a workflow path.
#[tauri::command]
pub fn workflow_list_runs(
    request: WorkflowListRunsRequest,
) -> Result<Vec<WorkflowRunRecord>, String> {
    let limit = request.limit.unwrap_or(20).clamp(1, 100);
    list_workflow_runs(Path::new(&request.root), &request.rel_path, limit)
        .map_err(|err| err.to_string())
}

/// Called from the workspace watcher on Created/Modified/Deleted/Renamed.
pub fn on_resource_changed(app: &AppHandle, root: &Path, changed_path: &str) {
    let Some(state) = app.try_state::<WorkflowState>() else {
        return;
    };
    set_active_workspace(&state, Some(root.to_path_buf()));

    {
        let mut debounce = match state.debounce.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let key = changed_path.to_string();
        let now = Instant::now();
        if let Some(last) = debounce.get(&key) {
            if now.duration_since(*last) < RESOURCE_CHANGE_DEBOUNCE {
                return;
            }
        }
        debounce.insert(key, now);
        // Bound map size.
        if debounce.len() > 256 {
            debounce.retain(|_, instant| now.duration_since(*instant) < Duration::from_secs(30));
        }
    }

    let app = app.clone();
    let root = root.to_path_buf();
    let changed = changed_path.to_string();
    thread::spawn(move || {
        let Some(state) = app.try_state::<WorkflowState>() else {
            return;
        };
        let workflows = match discover_workflows(&root) {
            Ok(list) => list,
            Err(err) => {
                eprintln!("lattice: workflow discovery failed: {err}");
                return;
            }
        };
        for (path, manifest) in workflows {
            if !manifest.matches_resource_change(&changed) {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            eprintln!("lattice: resource.changed matched workflow {rel} for {changed}");
            let _ = spawn_workflow_run(
                &app,
                &state,
                WorkflowRunRequest {
                    root: root.to_string_lossy().into_owned(),
                    rel_path: rel,
                    execution_id: None,
                    trigger: Some("resource.changed".into()),
                },
            );
        }
    });
}

/// Called after a successful form-backed insert_record.
pub fn on_form_submitted(app: &AppHandle, root: &Path, package_path: &str, form_name: &str) {
    let Some(state) = app.try_state::<WorkflowState>() else {
        return;
    };
    set_active_workspace(&state, Some(root.to_path_buf()));

    let form_file = format!(
        "{}/forms/{}.form.yaml",
        package_path.trim_end_matches('/'),
        form_name
    );

    let app = app.clone();
    let root = root.to_path_buf();
    let package = package_path.to_string();
    let form_name = form_name.to_string();
    thread::spawn(move || {
        let Some(state) = app.try_state::<WorkflowState>() else {
            return;
        };
        let workflows = match discover_workflows(&root) {
            Ok(list) => list,
            Err(err) => {
                eprintln!("lattice: workflow discovery failed: {err}");
                return;
            }
        };
        for (path, manifest) in workflows {
            if !manifest.matches_form_submitted(&package, &form_name, Some(&form_file)) {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            eprintln!("lattice: form.submitted matched workflow {rel} for {package}/{form_name}");
            let _ = spawn_workflow_run(
                &app,
                &state,
                WorkflowRunRequest {
                    root: root.to_string_lossy().into_owned(),
                    rel_path: rel,
                    execution_id: None,
                    trigger: Some("form.submitted".into()),
                },
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_tray_label_strips_suffix() {
        assert_eq!(
            workflow_tray_label("Automations/Contact intake.workflow.yaml"),
            "Contact intake"
        );
        assert_eq!(workflow_tray_label("Simple.workflow.yml"), "Simple");
    }

    #[test]
    fn manifest_view_exposes_trigger_type() {
        let view = WorkflowManifestView {
            format: "lattice-workflow".into(),
            version: 1,
            name: "Demo".into(),
            enabled: true,
            trigger: WorkflowTriggerView {
                trigger_type: "manual".into(),
                paths: None,
                form: None,
                package: None,
                form_id: None,
                interval_seconds: None,
                cron: None,
                timezone: None,
            },
            steps: vec![],
            raw_yaml: "format: lattice-workflow\n".into(),
        };
        let json = serde_json::to_value(&view).expect("serialize");
        assert_eq!(json["trigger"]["type"], "manual");
        assert_eq!(json["rawYaml"], "format: lattice-workflow\n");
    }
}
