//! Demo-driver / stage helpers for private ecosystem capture runs.
//!
//! Enabled only when `LATTICE_DEMO_DRIVER=1` (set by `exec-for-dev`). Never on
//! for normal shipped builds unless an operator explicitly exports the flag.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

use lattice_profile::{env_flag_enabled, LATTICE_DEMO_DRIVER_ENV, LATTICE_DEMO_SCENE_ENV};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoDriverConfig {
    pub enabled: bool,
    pub scene_path: Option<String>,
    pub scene: Option<Value>,
    pub stage_width: Option<u32>,
    pub stage_height: Option<u32>,
}

/// Whether the in-app demo driver should mount (exec-for-dev only by convention).
pub fn demo_driver_enabled() -> bool {
    env_flag_enabled(LATTICE_DEMO_DRIVER_ENV)
}

fn scene_path_from_env() -> Option<PathBuf> {
    let raw = std::env::var_os(LATTICE_DEMO_SCENE_ENV)?;
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    path.is_file().then_some(path)
}

/// Load demo-driver config for the desktop shell / capture harness.
pub fn load_demo_driver_config() -> DemoDriverConfig {
    let enabled = demo_driver_enabled();
    let Some(path) = scene_path_from_env() else {
        return DemoDriverConfig {
            enabled,
            scene_path: None,
            scene: None,
            stage_width: None,
            stage_height: None,
        };
    };
    let scene_path = Some(path.display().to_string());
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return DemoDriverConfig {
            enabled,
            scene_path,
            scene: None,
            stage_width: None,
            stage_height: None,
        };
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return DemoDriverConfig {
            enabled,
            scene_path,
            scene: None,
            stage_width: None,
            stage_height: None,
        };
    };
    let stage_width = value
        .pointer("/stage/width")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let stage_height = value
        .pointer("/stage/height")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    DemoDriverConfig {
        enabled,
        scene_path,
        scene: Some(value),
        stage_width,
        stage_height,
    }
}

#[tauri::command]
pub fn get_demo_driver_config() -> DemoDriverConfig {
    load_demo_driver_config()
}

/// Resize the main window to the scene stage (16:9 capture framing).
#[tauri::command]
pub fn apply_demo_stage(app: tauri::AppHandle) -> Result<DemoDriverConfig, String> {
    use tauri::{LogicalSize, Manager, Size};

    let config = load_demo_driver_config();
    let (Some(width), Some(height)) = (config.stage_width, config.stage_height) else {
        return Ok(config);
    };
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window missing".to_string())?;
    window
        .set_size(Size::Logical(LogicalSize::new(
            f64::from(width),
            f64::from(height),
        )))
        .map_err(|error| error.to_string())?;
    let _ = window.center();
    Ok(config)
}
