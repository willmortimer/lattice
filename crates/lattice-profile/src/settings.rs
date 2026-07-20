use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_storage::atomic_write_file;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

pub const DESKTOP_SETTINGS_FILENAME: &str = "desktop.yaml";
pub const WORKSPACE_SETTINGS_FILENAME: &str = "workspaces.yaml";

#[derive(Debug, Clone, Copy)]
pub struct SettingsSpec {
    pub filename: &'static str,
    pub format: &'static str,
    pub version: u32,
}

pub const DESKTOP_SETTINGS_SPEC: SettingsSpec = SettingsSpec {
    filename: DESKTOP_SETTINGS_FILENAME,
    format: "lattice-desktop-settings",
    version: 1,
};

pub const WORKSPACE_SETTINGS_SPEC: SettingsSpec = SettingsSpec {
    filename: WORKSPACE_SETTINGS_FILENAME,
    format: "lattice-workspace-settings",
    version: 1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDiagnostic {
    pub path: String,
    pub code: String,
    pub message: String,
    pub severity: SettingsDiagnosticSeverity,
}

#[derive(Debug, Clone)]
pub struct SettingsLoad<T> {
    pub value: T,
    pub revision: Option<String>,
    pub diagnostics: Vec<SettingsDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub desktop: DesktopSettings,
    pub workspaces: WorkspaceStartupSettings,
    pub desktop_revision: Option<String>,
    pub workspaces_revision: Option<String>,
    pub diagnostics: Vec<SettingsDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    root: PathBuf,
}

impl SettingsStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, spec: SettingsSpec) -> PathBuf {
        self.root.join(spec.filename)
    }

    pub fn load<T>(&self, spec: SettingsSpec) -> Result<SettingsLoad<T>>
    where
        T: Default + DeserializeOwned,
    {
        let path = self.path(spec);
        if !path.is_file() {
            return Ok(SettingsLoad {
                value: T::default(),
                revision: None,
                diagnostics: Vec::new(),
            });
        }
        let bytes = std::fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let revision = Some(content_revision(&bytes));
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(SettingsLoad {
                value: T::default(),
                revision,
                diagnostics: vec![diagnostic(
                    &path,
                    "settings-empty",
                    "The settings file is empty; defaults are active.",
                    SettingsDiagnosticSeverity::Warning,
                )],
            });
        }

        let value: serde_yaml::Value = match serde_yaml::from_slice(&bytes) {
            Ok(value) => value,
            Err(error) => {
                return Ok(SettingsLoad {
                    value: T::default(),
                    revision,
                    diagnostics: vec![diagnostic(
                        &path,
                        "settings-invalid-yaml",
                        format!("Could not parse settings; defaults are active: {error}"),
                        SettingsDiagnosticSeverity::Error,
                    )],
                });
            }
        };

        let format = value
            .get("format")
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_owned);
        let version = value.get("version").and_then(serde_yaml::Value::as_u64);
        if let Some(format) = format.as_deref() {
            if format != spec.format {
                return Ok(SettingsLoad {
                    value: T::default(),
                    revision,
                    diagnostics: vec![diagnostic(
                        &path,
                        "settings-format",
                        format!(
                            "Expected settings format {:?}, found {:?}; defaults are active.",
                            spec.format, format
                        ),
                        SettingsDiagnosticSeverity::Error,
                    )],
                });
            }
        }
        if version.is_some_and(|version| version > u64::from(spec.version)) {
            return Ok(SettingsLoad {
                value: T::default(),
                revision,
                diagnostics: vec![diagnostic(
                    &path,
                    "settings-newer-version",
                    format!(
                        "Settings version {} is newer than supported version {}; defaults are active and the file was preserved.",
                        version.unwrap(),
                        spec.version
                    ),
                    SettingsDiagnosticSeverity::Error,
                )],
            });
        }

        match serde_yaml::from_value(value) {
            Ok(value) => {
                let diagnostics = if format.is_none() || version.is_none() {
                    vec![diagnostic(
                        &path,
                        "settings-legacy",
                        "Legacy settings were loaded and will be upgraded on the next save.",
                        SettingsDiagnosticSeverity::Warning,
                    )]
                } else if version.is_some_and(|version| version < u64::from(spec.version)) {
                    vec![diagnostic(
                        &path,
                        "settings-old-version",
                        "Older settings were migrated in memory and will be upgraded on the next save.",
                        SettingsDiagnosticSeverity::Warning,
                    )]
                } else {
                    Vec::new()
                };
                Ok(SettingsLoad {
                    value,
                    revision,
                    diagnostics,
                })
            }
            Err(error) => Ok(SettingsLoad {
                value: T::default(),
                revision,
                diagnostics: vec![diagnostic(
                    &path,
                    "settings-invalid-shape",
                    format!("Settings have an invalid shape; defaults are active: {error}"),
                    SettingsDiagnosticSeverity::Error,
                )],
            }),
        }
    }

    /// Load settings and atomically materialize valid legacy/older documents
    /// in the current versioned format.
    ///
    /// Invalid, empty, differently formatted, and newer-version documents are
    /// never rewritten. If an upgrade cannot be persisted, the valid in-memory
    /// value remains active and a non-fatal diagnostic explains the failure.
    pub fn load_and_upgrade<T>(&self, spec: SettingsSpec) -> Result<SettingsLoad<T>>
    where
        T: Default + DeserializeOwned + Serialize,
    {
        let mut loaded = self.load::<T>(spec)?;
        let should_upgrade = !loaded.diagnostics.is_empty()
            && loaded.diagnostics.iter().all(|diagnostic| {
                matches!(
                    diagnostic.code.as_str(),
                    "settings-legacy" | "settings-old-version"
                )
            });
        if !should_upgrade {
            return Ok(loaded);
        }

        match self.save(spec, &loaded.value, loaded.revision.as_deref()) {
            Ok(revision) => {
                loaded.revision = Some(revision);
                loaded.diagnostics.clear();
            }
            // Another window may have completed the same migration first.
            // Reload so the caller sees the winner rather than a false error.
            Err(Error::RevisionConflict { .. }) => return self.load::<T>(spec),
            Err(error) => {
                loaded.diagnostics = vec![diagnostic(
                    &self.path(spec),
                    "settings-upgrade-failed",
                    format!(
                        "Legacy settings are active, but could not be upgraded on disk: {error}"
                    ),
                    SettingsDiagnosticSeverity::Warning,
                )];
            }
        }
        Ok(loaded)
    }

    pub fn save<T>(
        &self,
        spec: SettingsSpec,
        value: &T,
        expected_revision: Option<&str>,
    ) -> Result<String>
    where
        T: Serialize,
    {
        let path = self.path(spec);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let current = current_revision(&path)?;
        if let Some(expected) = expected_revision {
            let found = current.clone().unwrap_or_else(|| "missing".into());
            if found != expected {
                return Err(Error::RevisionConflict {
                    path,
                    expected: expected.to_string(),
                    found,
                });
            }
        }
        preserve_invalid_source(&path, spec)?;
        let bytes = serde_yaml::to_string(value)
            .map_err(|source| Error::Yaml {
                path: path.clone(),
                source,
            })?
            .into_bytes();
        atomic_write_file(&path, &bytes).map_err(|error| Error::Io {
            path: path.clone(),
            source: std::io::Error::other(error.to_string()),
        })?;
        Ok(content_revision(&bytes))
    }

    pub fn snapshot(&self) -> Result<SettingsSnapshot> {
        let desktop = self.load_and_upgrade::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)?;
        let workspaces =
            self.load_and_upgrade::<WorkspaceStartupSettings>(WORKSPACE_SETTINGS_SPEC)?;
        let mut diagnostics = desktop.diagnostics;
        diagnostics.extend(workspaces.diagnostics);
        Ok(SettingsSnapshot {
            desktop: desktop.value,
            workspaces: workspaces.value,
            desktop_revision: desktop.revision,
            workspaces_revision: workspaces.revision,
            diagnostics,
        })
    }
}

fn current_revision(path: &Path) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(content_revision(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn preserve_invalid_source(path: &Path, spec: SettingsSpec) -> Result<()> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(());
    };
    let valid = serde_yaml::from_slice::<serde_yaml::Value>(&bytes)
        .ok()
        .is_some_and(|value| {
            value
                .get("format")
                .and_then(serde_yaml::Value::as_str)
                .is_none_or(|format| format == spec.format)
                && value
                    .get("version")
                    .and_then(serde_yaml::Value::as_u64)
                    .is_none_or(|version| version <= u64::from(spec.version))
        });
    if valid {
        return Ok(());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let backup = path.with_extension(format!("invalid-{timestamp}.bak"));
    std::fs::copy(path, &backup).map_err(|source| Error::Io {
        path: backup,
        source,
    })?;
    Ok(())
}

fn content_revision(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn diagnostic(
    path: &Path,
    code: impl Into<String>,
    message: impl Into<String>,
    severity: SettingsDiagnosticSeverity,
) -> SettingsDiagnostic {
    SettingsDiagnostic {
        path: path.to_string_lossy().replace('\\', "/"),
        code: code.into(),
        message: message.into(),
        severity,
    }
}

fn desktop_format() -> String {
    DESKTOP_SETTINGS_SPEC.format.into()
}

fn desktop_version() -> u32 {
    DESKTOP_SETTINGS_SPEC.version
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    #[serde(default = "desktop_format")]
    pub format: String,
    #[serde(default = "desktop_version")]
    pub version: u32,
    #[serde(default)]
    pub editor: EditorSettings,
    #[serde(default)]
    pub files: FileSettings,
    #[serde(default)]
    pub keybindings: KeybindingSettings,
    #[serde(default)]
    pub data: DataSettings,
    #[serde(default)]
    pub performance: PerformanceSettings,
    #[serde(default)]
    pub diagnostics: DiagnosticSettings,
    #[serde(default)]
    pub services: ServicesSettings,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            format: desktop_format(),
            version: desktop_version(),
            editor: EditorSettings::default(),
            files: FileSettings::default(),
            keybindings: KeybindingSettings::default(),
            data: DataSettings::default(),
            performance: PerformanceSettings::default(),
            diagnostics: DiagnosticSettings::default(),
            services: ServicesSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicesSettings {
    /// When true, `latticed` stays running after the last client disconnects.
    #[serde(default)]
    pub keep_services_running: bool,
    /// When true, closing the main window hides it and keeps the process in the
    /// menu bar / tray (not a login item). Quit from the tray exits fully.
    #[serde(default)]
    pub keep_app_in_menu_bar: bool,
}

impl Default for ServicesSettings {
    fn default() -> Self {
        Self {
            keep_services_running: false,
            keep_app_in_menu_bar: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    #[serde(default = "default_autosave")]
    pub autosave_delay_ms: u64,
    #[serde(default = "yes")]
    pub spellcheck: bool,
    #[serde(default = "yes")]
    pub slash_commands: bool,
    #[serde(default = "yes")]
    pub show_frontmatter: bool,
    #[serde(default = "default_link_behavior")]
    pub link_click_behavior: String,
    #[serde(default = "default_page_width")]
    pub page_width: String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            autosave_delay_ms: default_autosave(),
            spellcheck: true,
            slash_commands: true,
            show_frontmatter: true,
            link_click_behavior: default_link_behavior(),
            page_width: default_page_width(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSettings {
    #[serde(default = "yes")]
    pub confirm_close_with_unsaved_changes: bool,
}

impl Default for FileSettings {
    fn default() -> Self {
        Self {
            confirm_close_with_unsaved_changes: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeybindingSettings {
    #[serde(default = "search_key")]
    pub search: String,
    #[serde(default = "palette_key")]
    pub command_palette: String,
    #[serde(default = "quick_note_key")]
    pub quick_note: String,
    #[serde(default = "new_page_key")]
    pub new_page: String,
    #[serde(default = "settings_key")]
    pub settings: String,
}

impl Default for KeybindingSettings {
    fn default() -> Self {
        Self {
            search: search_key(),
            command_palette: palette_key(),
            quick_note: quick_note_key(),
            new_page: new_page_key(),
            settings: settings_key(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSettings {
    #[serde(default = "comfortable")]
    pub row_height: String,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default = "yes")]
    pub show_row_numbers: bool,
    #[serde(default)]
    pub zebra_rows: bool,
    #[serde(default = "ascending")]
    pub default_sort_direction: String,
}

impl Default for DataSettings {
    fn default() -> Self {
        Self {
            row_height: comfortable(),
            page_size: default_page_size(),
            show_row_numbers: true,
            zebra_rows: false,
            default_sort_direction: ascending(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSettings {
    #[serde(default = "default_tabs")]
    pub max_open_tabs: u32,
    #[serde(default = "yes")]
    pub suspend_inactive_resources: bool,
    #[serde(default = "system")]
    pub reduced_motion: String,
    #[serde(default = "balanced")]
    pub renderer_cache: String,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_open_tabs: default_tabs(),
            suspend_inactive_resources: true,
            reduced_motion: system(),
            renderer_cache: balanced(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSettings {
    #[serde(default = "yes")]
    pub native_context_menus: bool,
    #[serde(default)]
    pub command_timings: bool,
    #[serde(default)]
    pub verbose_errors: bool,
    #[serde(default)]
    pub show_renderer_stats: bool,
}

impl Default for DiagnosticSettings {
    fn default() -> Self {
        Self {
            native_context_menus: true,
            command_timings: false,
            verbose_errors: false,
            show_renderer_stats: false,
        }
    }
}

fn workspace_format() -> String {
    WORKSPACE_SETTINGS_SPEC.format.into()
}

fn workspace_version() -> u32 {
    WORKSPACE_SETTINGS_SPEC.version
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStartupSettings {
    #[serde(default = "workspace_format")]
    pub format: String,
    #[serde(default = "workspace_version")]
    pub version: u32,
    #[serde(default)]
    pub default_workspace: Option<PathBuf>,
    #[serde(default = "yes")]
    pub reopen_last_workspace: bool,
    #[serde(default = "yes")]
    pub restore_session: bool,
    /// Brief branded splash before revealing the shell on launch.
    #[serde(default = "yes")]
    pub show_startup_splash: bool,
}

impl Default for WorkspaceStartupSettings {
    fn default() -> Self {
        Self {
            format: workspace_format(),
            version: workspace_version(),
            default_workspace: None,
            reopen_last_workspace: true,
            restore_session: true,
            show_startup_splash: true,
        }
    }
}

fn yes() -> bool {
    true
}
fn default_autosave() -> u64 {
    800
}
fn default_link_behavior() -> String {
    "navigate".into()
}
fn default_page_width() -> String {
    "standard".into()
}
fn search_key() -> String {
    "Mod+K".into()
}
fn palette_key() -> String {
    "Mod+P".into()
}
fn quick_note_key() -> String {
    "Mod+N".into()
}
fn new_page_key() -> String {
    "Mod+Shift+N".into()
}
fn settings_key() -> String {
    "Mod+,".into()
}
fn comfortable() -> String {
    "comfortable".into()
}
fn default_page_size() -> u32 {
    500
}
fn ascending() -> String {
    "asc".into()
}
fn default_tabs() -> u32 {
    12
}
fn system() -> String {
    "system".into()
}
fn balanced() -> String {
    "balanced".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_settings_degrade_and_are_backed_up_on_save() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path());
        let path = store.path(DESKTOP_SETTINGS_SPEC);
        std::fs::write(&path, "not: [valid").unwrap();

        let loaded = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert_eq!(loaded.value, DesktopSettings::default());
        assert_eq!(loaded.diagnostics[0].code, "settings-invalid-yaml");

        store
            .save(
                DESKTOP_SETTINGS_SPEC,
                &loaded.value,
                loaded.revision.as_deref(),
            )
            .unwrap();
        assert!(std::fs::read_dir(directory.path())
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("invalid-")));
    }

    #[test]
    fn newer_settings_are_preserved_and_defaults_are_used() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path());
        std::fs::write(
            store.path(DESKTOP_SETTINGS_SPEC),
            "format: lattice-desktop-settings\nversion: 99\n",
        )
        .unwrap();
        let loaded = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert_eq!(loaded.value, DesktopSettings::default());
        assert_eq!(loaded.diagnostics[0].code, "settings-newer-version");
    }

    #[test]
    fn missing_empty_and_legacy_settings_degrade_predictably() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path());

        let missing = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert_eq!(missing.value, DesktopSettings::default());
        assert!(missing.revision.is_none());
        assert!(missing.diagnostics.is_empty());

        std::fs::write(store.path(DESKTOP_SETTINGS_SPEC), " \n").unwrap();
        let empty = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert_eq!(empty.diagnostics[0].code, "settings-empty");

        std::fs::write(
            store.path(DESKTOP_SETTINGS_SPEC),
            "editor:\n  autosaveDelayMs: 1500\n",
        )
        .unwrap();
        let legacy = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert_eq!(legacy.value.editor.autosave_delay_ms, 1500);
        assert_eq!(legacy.diagnostics[0].code, "settings-legacy");
    }

    #[test]
    fn valid_legacy_settings_are_upgraded_atomically_on_load() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path());
        std::fs::write(
            store.path(DESKTOP_SETTINGS_SPEC),
            "editor:\n  autosaveDelayMs: 1500\n",
        )
        .unwrap();

        let loaded = store
            .load_and_upgrade::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert_eq!(loaded.value.editor.autosave_delay_ms, 1500);
        assert!(loaded.diagnostics.is_empty());
        let materialized = std::fs::read_to_string(store.path(DESKTOP_SETTINGS_SPEC)).unwrap();
        assert!(materialized.contains("format: lattice-desktop-settings"));
        assert!(materialized.contains("version: 1"));
    }

    #[test]
    fn optimistic_revisions_reject_stale_writes() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path());
        let revision = store
            .save(DESKTOP_SETTINGS_SPEC, &DesktopSettings::default(), None)
            .unwrap();
        std::fs::write(store.path(DESKTOP_SETTINGS_SPEC), "external: true\n").unwrap();
        assert!(matches!(
            store.save(
                DESKTOP_SETTINGS_SPEC,
                &DesktopSettings::default(),
                Some(&revision)
            ),
            Err(Error::RevisionConflict { .. })
        ));
    }

    #[test]
    fn services_menu_bar_residency_defaults_and_round_trips() {
        assert!(!ServicesSettings::default().keep_app_in_menu_bar);
        assert!(!ServicesSettings::default().keep_services_running);

        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path());
        std::fs::write(
            store.path(DESKTOP_SETTINGS_SPEC),
            "format: lattice-desktop-settings\nversion: 1\nservices:\n  keepServicesRunning: true\n  keepAppInMenuBar: true\n",
        )
        .unwrap();
        let loaded = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert!(loaded.value.services.keep_services_running);
        assert!(loaded.value.services.keep_app_in_menu_bar);

        // Older documents without the new field keep the default (false).
        std::fs::write(
            store.path(DESKTOP_SETTINGS_SPEC),
            "format: lattice-desktop-settings\nversion: 1\nservices:\n  keepServicesRunning: true\n",
        )
        .unwrap();
        let partial = store
            .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
            .unwrap();
        assert!(partial.value.services.keep_services_running);
        assert!(!partial.value.services.keep_app_in_menu_bar);
    }
}
