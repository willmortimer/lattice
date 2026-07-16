use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_core::{effective_default_workspace, ensure_lattice_home, Workspace};
use lattice_profile::{
    DesktopSession, DesktopSettings, RecentWorkspace, SettingsSnapshot, WorkspaceStartupSettings,
    DESKTOP_SETTINGS_SPEC, WORKSPACE_SETTINGS_SPEC,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSnapshot {
    pub settings: SettingsSnapshot,
    pub recents: Vec<RecentWorkspace>,
    pub sidebar_width: Option<f64>,
    pub effective_default_workspace: Option<String>,
    pub has_valid_configured_default: bool,
}

fn snapshot() -> Result<ProfileSnapshot, String> {
    let home = ensure_lattice_home().map_err(err)?;
    let settings = home.settings_store().snapshot().map_err(err)?;
    let state = home.state_store().map_err(err)?;
    let recents = state.list_recents().map_err(err)?;
    let sidebar_width = state
        .ui_value("sidebar-width")
        .map_err(err)?
        .and_then(|value| value.parse().ok());
    let effective_default_workspace = effective_default_workspace(&home)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let has_valid_configured_default = home
        .configured_default_workspace()
        .ok()
        .flatten()
        .is_some_and(|path| Workspace::open(&path).is_ok());
    Ok(ProfileSnapshot {
        settings,
        recents,
        sidebar_width,
        effective_default_workspace,
        has_valid_configured_default,
    })
}

#[tauri::command]
pub fn get_profile_snapshot() -> Result<ProfileSnapshot, String> {
    snapshot()
}

#[tauri::command]
pub fn save_desktop_settings(
    settings: DesktopSettings,
    expected_revision: Option<String>,
) -> Result<ProfileSnapshot, String> {
    let home = ensure_lattice_home().map_err(err)?;
    home.settings_store()
        .save(
            DESKTOP_SETTINGS_SPEC,
            &settings,
            expected_revision.as_deref(),
        )
        .map_err(err)?;
    snapshot()
}

#[tauri::command]
pub fn save_workspace_startup_settings(
    mut settings: WorkspaceStartupSettings,
    expected_revision: Option<String>,
) -> Result<ProfileSnapshot, String> {
    if let Some(path) = settings.default_workspace.as_deref() {
        Workspace::open(path).map_err(err)?;
        settings.default_workspace = Some(std::fs::canonicalize(path).map_err(err)?);
    }
    let home = ensure_lattice_home().map_err(err)?;
    home.settings_store()
        .save(
            WORKSPACE_SETTINGS_SPEC,
            &settings,
            expected_revision.as_deref(),
        )
        .map_err(err)?;
    snapshot()
}

#[tauri::command]
pub fn remember_workspace(root: String, title: String) -> Result<Vec<RecentWorkspace>, String> {
    Workspace::open(Path::new(&root)).map_err(err)?;
    let home = ensure_lattice_home().map_err(err)?;
    let mut state = home.state_store().map_err(err)?;
    state
        .remember_workspace(&RecentWorkspace {
            root,
            title,
            opened_at: now(),
        })
        .map_err(err)?;
    state.list_recents().map_err(err)
}

#[tauri::command]
pub fn clear_recent_workspaces() -> Result<Vec<RecentWorkspace>, String> {
    let home = ensure_lattice_home().map_err(err)?;
    let state = home.state_store().map_err(err)?;
    state.clear_recents().map_err(err)?;
    Ok(Vec::new())
}

#[tauri::command]
pub fn remove_recent_workspace(root: String) -> Result<Vec<RecentWorkspace>, String> {
    let home = ensure_lattice_home().map_err(err)?;
    let state = home.state_store().map_err(err)?;
    state.remove_recent(&root).map_err(err)?;
    state.list_recents().map_err(err)
}

#[tauri::command]
pub fn load_desktop_session(root: String) -> Result<Option<DesktopSession>, String> {
    let home = ensure_lattice_home().map_err(err)?;
    home.state_store()
        .map_err(err)?
        .load_session(&root)
        .map_err(err)
}

#[tauri::command]
pub fn save_desktop_session(session: DesktopSession) -> Result<(), String> {
    let home = ensure_lattice_home().map_err(err)?;
    home.state_store()
        .map_err(err)?
        .save_session(&session)
        .map_err(err)
}

#[tauri::command]
pub fn set_profile_ui_value(key: String, value: String) -> Result<(), String> {
    let home = ensure_lattice_home().map_err(err)?;
    home.state_store()
        .map_err(err)?
        .set_ui_value(&key, &value)
        .map_err(err)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyProfileImport {
    pub desktop_settings: Option<serde_json::Value>,
    #[serde(default)]
    pub recents: Vec<RecentWorkspace>,
    #[serde(default)]
    pub sessions: Vec<DesktopSession>,
    pub sidebar_width: Option<f64>,
}

#[tauri::command]
pub fn import_legacy_profile(payload: LegacyProfileImport) -> Result<ProfileSnapshot, String> {
    const MIGRATION: &str = "webview-local-storage-v1";
    let home = ensure_lattice_home().map_err(err)?;
    let mut state = home.state_store().map_err(err)?;
    if state.migration_completed(MIGRATION).map_err(err)? {
        return snapshot();
    }

    let store = home.settings_store();
    let current = store.snapshot().map_err(err)?;
    if current.desktop_revision.is_none() {
        if let Some(value) = payload.desktop_settings.as_ref() {
            let desktop = serde_json::from_value::<DesktopSettings>(value.clone())
                .unwrap_or_else(|_| DesktopSettings::default());
            store
                .save(DESKTOP_SETTINGS_SPEC, &desktop, None)
                .map_err(err)?;
        }
    }
    if current.workspaces_revision.is_none() {
        let mut workspaces = WorkspaceStartupSettings::default();
        if let Some(files) = payload
            .desktop_settings
            .as_ref()
            .and_then(|value| value.get("files"))
        {
            if let Some(value) = files
                .get("reopenLastWorkspace")
                .and_then(|value| value.as_bool())
            {
                workspaces.reopen_last_workspace = value;
            }
            if let Some(value) = files
                .get("restoreSession")
                .and_then(|value| value.as_bool())
            {
                workspaces.restore_session = value;
            }
        }
        store
            .save(WORKSPACE_SETTINGS_SPEC, &workspaces, None)
            .map_err(err)?;
    }

    for recent in &payload.recents {
        state.remember_workspace(recent).map_err(err)?;
    }
    for session in &payload.sessions {
        state.save_session(session).map_err(err)?;
    }
    if let Some(width) = payload.sidebar_width {
        state
            .set_ui_value("sidebar-width", &width.to_string())
            .map_err(err)?;
    }
    state.complete_migration(MIGRATION).map_err(err)?;
    snapshot()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn err(error: impl ToString) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn legacy_profile_import_runs_once_and_persists_native_state() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", directory.path());
        let payload = LegacyProfileImport {
            desktop_settings: Some(serde_json::json!({
                "editor": { "autosaveDelayMs": 1500 }
            })),
            recents: vec![RecentWorkspace {
                root: "/tmp/example".into(),
                title: "Example".into(),
                opened_at: 10,
            }],
            sessions: vec![DesktopSession {
                root: "/tmp/example".into(),
                tabs: vec!["Home.md".into()],
                active: Some("Home.md".into()),
                activity: Some("files".into()),
                inspector: false,
            }],
            sidebar_width: Some(320.0),
        };
        let imported = import_legacy_profile(payload).unwrap();
        assert_eq!(imported.settings.desktop.editor.autosave_delay_ms, 1500);
        assert_eq!(imported.recents[0].title, "Example");
        assert_eq!(imported.sidebar_width, Some(320.0));

        let second = import_legacy_profile(LegacyProfileImport {
            desktop_settings: None,
            recents: Vec::new(),
            sessions: Vec::new(),
            sidebar_width: Some(400.0),
        })
        .unwrap();
        assert_eq!(second.sidebar_width, Some(320.0));
        std::env::remove_var("LATTICE_HOME");
    }
}
