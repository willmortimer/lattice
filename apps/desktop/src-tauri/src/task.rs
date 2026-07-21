//! Tauri-side wiring for `*.task/` packages (open / run / cancel).
//!
//! Keeps `lattice-commands` free of Tauri: this module owns execution state,
//! workspace containment, background spawn, and event emission.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_commands::{
    ExecutionResult, ExecutionStatus, ResourceOutput, SpawnedTask, TaskIoRef, TaskManifest,
    TaskRunner, TASK_MANIFEST_FILENAME,
};
use lattice_core::Workspace;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const TASK_EXECUTION_EVENT: &str = "task-execution-updated";

/// Tauri-managed map of in-flight and completed task executions.
#[derive(Default)]
pub struct TaskState(Mutex<HashMap<String, LiveExecution>>);

struct LiveExecution {
    result: Arc<Mutex<ExecutionResult>>,
    spawned: Arc<Mutex<Option<SpawnedTask>>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLoadRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunRequest {
    pub root: String,
    pub rel_path: String,
    #[serde(default)]
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunResponse {
    pub execution_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionRequest {
    pub execution_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskIoView {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRuntimeView {
    #[serde(rename = "type")]
    pub runtime_type: String,
    pub provider: String,
    pub project: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEntrypointView {
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLimitsView {
    pub timeout_seconds: u64,
}

/// CamelCase manifest DTO for the desktop shell (YAML stays snake_case).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskManifestView {
    pub format: String,
    pub version: u32,
    pub runtime: TaskRuntimeView,
    pub entrypoint: TaskEntrypointView,
    pub limits: TaskLimitsView,
    pub inputs: Vec<TaskIoView>,
    pub outputs: Vec<TaskIoView>,
}

fn io_view(value: &TaskIoRef) -> TaskIoView {
    TaskIoView {
        path: value.path().to_string(),
        kind: value.kind().map(str::to_string),
    }
}

fn manifest_view(manifest: TaskManifest) -> TaskManifestView {
    TaskManifestView {
        format: manifest.format,
        version: manifest.version,
        runtime: TaskRuntimeView {
            runtime_type: manifest.runtime.runtime_type,
            provider: manifest.runtime.provider,
            project: manifest.runtime.project,
        },
        entrypoint: TaskEntrypointView {
            command: manifest.entrypoint.command,
        },
        limits: TaskLimitsView {
            timeout_seconds: manifest.limits.timeout_seconds,
        },
        inputs: manifest.inputs.iter().map(io_view).collect(),
        outputs: manifest.outputs.iter().map(io_view).collect(),
    }
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    lattice_runtime::rfc3339_utc(secs)
}

fn resource_outputs(declared: &[TaskIoRef]) -> Vec<ResourceOutput> {
    declared
        .iter()
        .map(|io| ResourceOutput {
            path: io.path().to_string(),
            kind: io.kind().map(str::to_string),
            hash: None,
        })
        .collect()
}

fn open_workspace(root: &Path) -> Result<Workspace, String> {
    Workspace::open(root).map_err(|err| err.to_string())
}

fn resolve_package(workspace: &Workspace, rel_path: &str) -> Result<PathBuf, String> {
    let package = workspace.root().join(rel_path);
    if !package.exists() {
        return Err(format!("task package not found: {rel_path}"));
    }
    // Containment: package must stay under the workspace root.
    let canonical_root = workspace
        .root()
        .canonicalize()
        .map_err(|err| err.to_string())?;
    let canonical_pkg = package.canonicalize().map_err(|err| err.to_string())?;
    if !canonical_pkg.starts_with(&canonical_root) {
        return Err("task path escapes workspace root".into());
    }
    Ok(canonical_pkg)
}

fn update_and_emit(app: &AppHandle, result: &Arc<Mutex<ExecutionResult>>) {
    let snapshot = match result.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };
    if let Err(err) = app.emit(TASK_EXECUTION_EVENT, &snapshot) {
        eprintln!("lattice: failed to emit {TASK_EXECUTION_EVENT}: {err}");
    }
}

fn patch_result(result: &Arc<Mutex<ExecutionResult>>, patch: impl FnOnce(&mut ExecutionResult)) {
    if let Ok(mut guard) = result.lock() {
        patch(&mut guard);
    }
}

/// Load and validate `task.yaml` for a workspace-relative `.task/` package.
#[tauri::command]
pub fn task_load_manifest(request: TaskLoadRequest) -> Result<TaskManifestView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let package = resolve_package(&workspace, &request.rel_path)?;
    let manifest_path = if package.is_file() {
        package
    } else {
        package.join(TASK_MANIFEST_FILENAME)
    };
    let manifest = TaskManifest::load(&manifest_path).map_err(|err| err.to_string())?;
    Ok(manifest_view(manifest))
}

/// Start a background task run; returns immediately with an execution id.
#[tauri::command]
pub fn task_run(
    request: TaskRunRequest,
    app: AppHandle,
    state: tauri::State<'_, TaskState>,
) -> Result<TaskRunResponse, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let package = resolve_package(&workspace, &request.rel_path)?;
    let execution_id = request
        .execution_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    let started_at = now_iso();
    let result = Arc::new(Mutex::new(ExecutionResult {
        id: execution_id.clone(),
        status: ExecutionStatus::Running,
        stdout: String::new(),
        stderr: String::new(),
        started_at,
        finished_at: None,
        outputs: Vec::new(),
        proposal_id: None,
    }));
    let spawned_slot: Arc<Mutex<Option<SpawnedTask>>> = Arc::new(Mutex::new(None));
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut map = state.0.lock().map_err(|_| "task state lock poisoned")?;
        map.insert(
            execution_id.clone(),
            LiveExecution {
                result: Arc::clone(&result),
                spawned: Arc::clone(&spawned_slot),
                cancel: Arc::clone(&cancel),
            },
        );
    }

    update_and_emit(&app, &result);

    let app_thread = app.clone();
    let result_thread = Arc::clone(&result);
    let spawned_thread = Arc::clone(&spawned_slot);
    let cancel_thread = Arc::clone(&cancel);

    thread::spawn(move || {
        let runner = TaskRunner::new();
        let spawned = match runner.spawn(&package) {
            Ok(spawned) => spawned,
            Err(err) => {
                patch_result(&result_thread, |r| {
                    r.status = ExecutionStatus::Failed;
                    r.stderr = err.to_string();
                    r.finished_at = Some(now_iso());
                });
                update_and_emit(&app_thread, &result_thread);
                return;
            }
        };

        let declared = spawned.declared_outputs.clone();
        if let Ok(mut slot) = spawned_thread.lock() {
            *slot = Some(spawned);
        } else {
            patch_result(&result_thread, |r| {
                r.status = ExecutionStatus::Failed;
                r.stderr = "task state lock poisoned".into();
                r.finished_at = Some(now_iso());
            });
            update_and_emit(&app_thread, &result_thread);
            return;
        }

        // Re-borrow from the slot so cancel can race against wait.
        loop {
            if cancel_thread.load(Ordering::SeqCst) {
                let mut slot = match spawned_thread.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if let Some(mut child) = slot.take() {
                    let _ = child.kill();
                    let out = child.wait_after_kill();
                    patch_result(&result_thread, |r| {
                        r.status = ExecutionStatus::Cancelled;
                        r.stdout = out.stdout;
                        r.stderr = out.stderr;
                        r.finished_at = Some(now_iso());
                        r.outputs = resource_outputs(&declared);
                    });
                } else {
                    patch_result(&result_thread, |r| {
                        r.status = ExecutionStatus::Cancelled;
                        r.finished_at = Some(now_iso());
                    });
                }
                update_and_emit(&app_thread, &result_thread);
                return;
            }

            let poll = {
                let mut slot = match spawned_thread.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                match slot.as_mut() {
                    Some(child) => {
                        patch_result(&result_thread, |r| {
                            r.stdout = child.stdout_snapshot();
                            r.stderr = child.stderr_snapshot();
                        });
                        child.try_finish()
                    }
                    None => break,
                }
            };

            match poll {
                None => {
                    update_and_emit(&app_thread, &result_thread);
                    thread::sleep(std::time::Duration::from_millis(50));
                }
                Some(Ok(out)) => {
                    let status = if out.exit_code == 0 {
                        ExecutionStatus::Succeeded
                    } else {
                        ExecutionStatus::Failed
                    };
                    if let Ok(mut slot) = spawned_thread.lock() {
                        let _ = slot.take();
                    }
                    patch_result(&result_thread, |r| {
                        r.status = status;
                        r.stdout = out.stdout;
                        r.stderr = out.stderr;
                        r.finished_at = Some(now_iso());
                        r.outputs = resource_outputs(&declared);
                    });
                    update_and_emit(&app_thread, &result_thread);
                    return;
                }
                Some(Err(err)) => {
                    if let Ok(mut slot) = spawned_thread.lock() {
                        let _ = slot.take();
                    }
                    let (stdout, stderr, status) = match err {
                        lattice_commands::TaskError::TimedOut {
                            stdout,
                            stderr,
                            timeout_seconds,
                        } => (
                            stdout,
                            format!("task timed out after {timeout_seconds}s\n{stderr}"),
                            ExecutionStatus::Failed,
                        ),
                        other => (String::new(), other.to_string(), ExecutionStatus::Failed),
                    };
                    patch_result(&result_thread, |r| {
                        r.status = status;
                        r.stdout = stdout;
                        r.stderr = stderr;
                        r.finished_at = Some(now_iso());
                        r.outputs = resource_outputs(&declared);
                    });
                    update_and_emit(&app_thread, &result_thread);
                    return;
                }
            }
        }
    });

    Ok(TaskRunResponse { execution_id })
}

/// Process-group kill an in-flight execution.
#[tauri::command]
pub fn task_cancel(
    request: TaskExecutionRequest,
    state: tauri::State<'_, TaskState>,
) -> Result<(), String> {
    let map = state.0.lock().map_err(|_| "task state lock poisoned")?;
    let live = map
        .get(&request.execution_id)
        .ok_or_else(|| format!("unknown task execution: {}", request.execution_id))?;
    live.cancel.store(true, Ordering::SeqCst);
    if let Ok(mut slot) = live.spawned.lock() {
        if let Some(child) = slot.as_mut() {
            let _ = child.kill();
        }
    }
    Ok(())
}

/// Poll the current [`ExecutionResult`] for an execution id.
#[tauri::command]
pub fn task_execution_status(
    request: TaskExecutionRequest,
    state: tauri::State<'_, TaskState>,
) -> Result<ExecutionResult, String> {
    let map = state.0.lock().map_err(|_| "task state lock poisoned")?;
    let live = map
        .get(&request.execution_id)
        .ok_or_else(|| format!("unknown task execution: {}", request.execution_id))?;
    live.result
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "task result lock poisoned".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_view_uses_camel_case_timeout() {
        let view = TaskManifestView {
            format: "lattice-task".into(),
            version: 1,
            runtime: TaskRuntimeView {
                runtime_type: "python".into(),
                provider: "uv".into(),
                project: ".".into(),
            },
            entrypoint: TaskEntrypointView {
                command: vec!["python".into(), "main.py".into()],
            },
            limits: TaskLimitsView {
                timeout_seconds: 60,
            },
            inputs: vec![TaskIoView {
                path: "Data/in.csv".into(),
                kind: None,
            }],
            outputs: vec![],
        };
        let json = serde_json::to_value(&view).expect("serialize");
        assert_eq!(json["limits"]["timeoutSeconds"], 60);
        assert_eq!(json["runtime"]["type"], "python");
        assert_eq!(json["inputs"][0]["path"], "Data/in.csv");
    }
}
