//! Tauri wiring for `*.derived.yaml` resources (status + rebuild).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use lattice_commands::{
    load_derived_status, rebuild_derived, DerivedInputHash, DerivedManifest, DerivedState,
    DerivedStatus, TaskRunner,
};
use lattice_core::Workspace;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const DERIVED_STATUS_EVENT: &str = "derived-status-updated";

/// In-flight rebuild tracking (resource path → latest status).
#[derive(Default)]
pub struct DerivedStateMap(Mutex<std::collections::HashMap<String, Arc<Mutex<DerivedStatusView>>>>);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedLoadRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedRebuildRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedInputView {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedManifestView {
    pub format: String,
    pub version: u32,
    pub output: String,
    pub inputs: Vec<String>,
    pub builder_task: String,
    pub refresh_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedStatusView {
    pub resource_path: String,
    pub state: String,
    pub output: String,
    pub builder_task: String,
    pub refresh_mode: String,
    pub inputs: Vec<DerivedInputView>,
    pub current_inputs: Vec<DerivedInputView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_built_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

fn input_view(value: &DerivedInputHash) -> DerivedInputView {
    DerivedInputView {
        path: value.path.clone(),
        hash: value.hash.clone(),
        pattern: value.pattern.clone(),
    }
}

fn state_str(state: DerivedState) -> String {
    state.as_str().to_string()
}

fn status_view(status: DerivedStatus) -> DerivedStatusView {
    DerivedStatusView {
        resource_path: status.resource_path,
        state: state_str(status.state),
        output: status.output,
        builder_task: status.builder_task,
        refresh_mode: status.refresh_mode,
        inputs: status.inputs.iter().map(input_view).collect(),
        current_inputs: status.current_inputs.iter().map(input_view).collect(),
        last_built_at: status.last_built_at,
        last_error: status.last_error,
    }
}

fn manifest_view(manifest: DerivedManifest) -> DerivedManifestView {
    DerivedManifestView {
        format: manifest.format,
        version: manifest.version,
        output: manifest.output,
        inputs: manifest.inputs,
        builder_task: manifest.builder.task,
        refresh_mode: manifest.refresh.mode,
    }
}

fn open_workspace(root: &Path) -> Result<Workspace, String> {
    Workspace::open(root).map_err(|err| err.to_string())
}

fn resolve_derived(workspace: &Workspace, rel_path: &str) -> Result<PathBuf, String> {
    let path = workspace.root().join(rel_path);
    if !path.is_file() {
        return Err(format!("derived resource not found: {rel_path}"));
    }
    let canonical_root = workspace
        .root()
        .canonicalize()
        .map_err(|err| err.to_string())?;
    let canonical = path.canonicalize().map_err(|err| err.to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("derived path escapes workspace root".into());
    }
    Ok(canonical)
}

/// Load and validate a `*.derived.yaml` manifest.
#[tauri::command]
pub fn derived_load_manifest(request: DerivedLoadRequest) -> Result<DerivedManifestView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let path = resolve_derived(&workspace, &request.rel_path)?;
    let manifest = DerivedManifest::load(&path).map_err(|err| err.to_string())?;
    Ok(manifest_view(manifest))
}

/// Compute live derived status (current / stale / building / failed).
#[tauri::command]
pub fn derived_load_status(request: DerivedLoadRequest) -> Result<DerivedStatusView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let _ = resolve_derived(&workspace, &request.rel_path)?;
    let status =
        load_derived_status(workspace.root(), &request.rel_path).map_err(|err| err.to_string())?;
    Ok(status_view(status))
}

/// Rebuild the derived resource via its declared builder task (background).
#[tauri::command]
pub fn derived_rebuild(
    request: DerivedRebuildRequest,
    app: AppHandle,
    state: tauri::State<'_, DerivedStateMap>,
) -> Result<DerivedStatusView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let _ = resolve_derived(&workspace, &request.rel_path)?;

    // Optimistic building status for the UI.
    let mut building =
        load_derived_status(workspace.root(), &request.rel_path).map_err(|err| err.to_string())?;
    building.state = DerivedState::Building;
    let view = status_view(building);
    let slot = Arc::new(Mutex::new(view.clone()));
    {
        let mut map = state.0.lock().map_err(|_| "derived state lock poisoned")?;
        map.insert(request.rel_path.clone(), Arc::clone(&slot));
    }
    let _ = app.emit(DERIVED_STATUS_EVENT, &view);

    let root = workspace.root().to_path_buf();
    let rel = request.rel_path.clone();
    let app_thread = app.clone();
    let slot_thread = Arc::clone(&slot);

    thread::spawn(move || {
        let runner = TaskRunner::new();
        let result = rebuild_derived(&root, &rel, &runner);
        let next = match result {
            Ok(status) => status_view(status),
            Err(err) => {
                let mut failed = match load_derived_status(&root, &rel) {
                    Ok(status) => status_view(status),
                    Err(_) => DerivedStatusView {
                        resource_path: rel.clone(),
                        state: "failed".into(),
                        output: String::new(),
                        builder_task: String::new(),
                        refresh_mode: "on-demand".into(),
                        inputs: Vec::new(),
                        current_inputs: Vec::new(),
                        last_built_at: None,
                        last_error: Some(err.to_string()),
                    },
                };
                if failed.last_error.is_none() {
                    failed.last_error = Some(err.to_string());
                }
                failed.state = "failed".into();
                failed
            }
        };
        if let Ok(mut guard) = slot_thread.lock() {
            *guard = next.clone();
        }
        let _ = app_thread.emit(DERIVED_STATUS_EVENT, &next);
    });

    Ok(view)
}
