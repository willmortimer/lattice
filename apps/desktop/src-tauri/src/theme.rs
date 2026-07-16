//! Theme / appearance Tauri commands and settings-directory watch.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use lattice_theme::{
    discover_themes, load_appearance, resolve_active_theme, save_appearance, AppearanceMode,
    ResolvedTheme, SystemAppearance, ThemeDiagnostic, ThemeSummary,
};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

const THEME_CHANGED_EVENT: &str = "theme-changed";

#[derive(Default)]
pub struct ThemeWatchState(Mutex<Option<Debouncer<RecommendedWatcher, RecommendedCache>>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeCatalog {
    pub themes: Vec<ThemeSummary>,
    pub diagnostics: Vec<ThemeDiagnostic>,
    pub resolved: ResolvedTheme,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveThemeArgs {
    /// `dark` or `light` from the frontend (prefers-color-scheme / window theme).
    pub system: String,
    /// Optional open workspace root for `.lattice/theme.yaml`.
    pub workspace_root: Option<String>,
}

fn system_from_str(s: &str) -> SystemAppearance {
    match s.to_ascii_lowercase().as_str() {
        "light" => SystemAppearance::Light,
        _ => SystemAppearance::Dark,
    }
}

fn err_string(e: impl ToString) -> String {
    e.to_string()
}

fn catalog(
    system: SystemAppearance,
    workspace_root: Option<&Path>,
) -> Result<ThemeCatalog, String> {
    let (home, settings) = load_appearance().map_err(err_string)?;
    let (themes, diagnostics) = discover_themes(&home).map_err(err_string)?;
    let mut resolved =
        resolve_active_theme(&home, &settings, system, workspace_root).map_err(err_string)?;
    resolved.diagnostics.extend(diagnostics.iter().cloned());
    Ok(ThemeCatalog {
        themes,
        diagnostics,
        resolved,
    })
}

#[tauri::command]
pub fn list_themes(system: String, workspace_root: Option<String>) -> Result<ThemeCatalog, String> {
    catalog(
        system_from_str(&system),
        workspace_root.as_deref().map(Path::new),
    )
}

#[tauri::command]
pub fn get_resolved_theme(args: ResolveThemeArgs) -> Result<ResolvedTheme, String> {
    let (home, settings) = load_appearance().map_err(err_string)?;
    resolve_active_theme(
        &home,
        &settings,
        system_from_str(&args.system),
        args.workspace_root.as_deref().map(Path::new),
    )
    .map_err(err_string)
}

#[tauri::command]
pub fn set_theme(
    theme_id: String,
    system: String,
    workspace_root: Option<String>,
    app: AppHandle,
) -> Result<ThemeCatalog, String> {
    let (_home, mut settings) = load_appearance().map_err(err_string)?;
    settings.mode = AppearanceMode::Fixed;
    settings.theme = theme_id;
    save_appearance(&settings).map_err(err_string)?;
    let catalog = catalog(
        system_from_str(&system),
        workspace_root.as_deref().map(Path::new),
    )?;
    let _ = app.emit(THEME_CHANGED_EVENT, &catalog.resolved);
    Ok(catalog)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAppearanceModeArgs {
    pub mode: String,
    pub theme: Option<String>,
    pub pair_dark: Option<String>,
    pub pair_light: Option<String>,
    pub system: String,
    pub workspace_root: Option<String>,
}

#[tauri::command]
pub fn set_appearance_mode(
    args: SetAppearanceModeArgs,
    app: AppHandle,
) -> Result<ThemeCatalog, String> {
    let (_home, mut settings) = load_appearance().map_err(err_string)?;
    settings.mode = match args.mode.to_ascii_lowercase().as_str() {
        "auto" => AppearanceMode::Auto,
        _ => AppearanceMode::Fixed,
    };
    if let Some(theme) = args.theme {
        settings.theme = theme;
    }
    if let Some(dark) = args.pair_dark {
        settings.pair.dark = dark;
    }
    if let Some(light) = args.pair_light {
        settings.pair.light = light;
    }
    save_appearance(&settings).map_err(err_string)?;
    let catalog = catalog(
        system_from_str(&args.system),
        args.workspace_root.as_deref().map(Path::new),
    )?;
    let _ = app.emit(THEME_CHANGED_EVENT, &catalog.resolved);
    Ok(catalog)
}

/// Start watching `~/Lattice/Settings` (appearance + user themes) and optional
/// workspace `.lattice/`.
#[tauri::command]
pub fn start_theme_watching(
    workspace_root: Option<String>,
    app: AppHandle,
    state: State<ThemeWatchState>,
) {
    stop_theme_watching(state.clone());

    let (home, _) = match load_appearance() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("lattice: theme watch: {err}");
            return;
        }
    };

    let settings_dir = home.settings.clone();
    let themes_dir = settings_dir.join("themes");
    let _ = std::fs::create_dir_all(&themes_dir);

    let app_handle = app.clone();
    let mut debouncer = match new_debouncer(
        Duration::from_millis(300),
        None,
        move |result: DebounceEventResult| {
            if let Err(errors) = result {
                for err in errors {
                    eprintln!("lattice: theme watch: {err}");
                }
                return;
            }
            let payload = ThemeWatchPing {
                reason: "settings-changed".into(),
            };
            let _ = app_handle.emit(THEME_CHANGED_EVENT, payload);
        },
    ) {
        Ok(d) => d,
        Err(err) => {
            eprintln!("lattice: theme watch start failed: {err}");
            return;
        }
    };

    if let Err(err) = debouncer.watch(&settings_dir, RecursiveMode::Recursive) {
        eprintln!("lattice: theme watch settings: {err}");
    }

    if let Some(root) = workspace_root {
        let lattice_dir = PathBuf::from(root).join(".lattice");
        if lattice_dir.is_dir() {
            if let Err(err) = debouncer.watch(&lattice_dir, RecursiveMode::NonRecursive) {
                eprintln!("lattice: theme watch workspace: {err}");
            }
        }
    }

    *state.0.lock().expect("theme watch lock") = Some(debouncer);
}

#[tauri::command]
pub fn stop_theme_watching(state: State<ThemeWatchState>) {
    *state.0.lock().expect("theme watch lock") = None;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThemeWatchPing {
    reason: String,
}
