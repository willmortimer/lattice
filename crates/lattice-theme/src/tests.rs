use std::sync::Mutex;

use tempfile::tempdir;

use crate::appearance::{
    ensure_user_themes_dir, save_appearance, AppearanceMode, AppearanceSettings,
};
use crate::builtin::load_builtin;
use crate::discover::{check_theme_file, discover_themes, load_theme_by_id};
use crate::document::builtin_path;
use crate::flatten::flatten_theme;
use crate::override_file::{load_workspace_override, WorkspaceThemeOverride};
use crate::resolve::{resolve_active_theme, SystemAppearance};
use lattice_core::ensure_lattice_home;

/// `LATTICE_HOME` is process-global; serialize tests that mutate it.
static HOME_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn builtins_parse_and_flatten() {
    for id in crate::builtin::BUILTIN_IDS {
        let doc = load_builtin(id).unwrap();
        let vars = flatten_theme(&doc, &builtin_path(id)).unwrap();
        assert!(vars.contains_key("--lt-bg"));
        assert!(vars.contains_key("--lt-accent"));
        assert!(vars["--lt-accent-wash"].contains("color-mix"));
    }
}

#[test]
fn slate_ground_matches_shipped_default() {
    let doc = load_builtin("lattice-slate").unwrap();
    let vars = flatten_theme(&doc, &builtin_path("lattice-slate")).unwrap();
    assert_eq!(vars.get("--lt-bg").map(String::as_str), Some("#0a0d13"));
}

#[test]
fn appearance_round_trip() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let settings = AppearanceSettings {
        mode: AppearanceMode::Auto,
        pair: crate::appearance::ThemePair {
            light: "lattice-paper".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    save_appearance(&settings).unwrap();
    let (_home, loaded) = crate::appearance::load_appearance().unwrap();
    assert_eq!(loaded.mode, AppearanceMode::Auto);
    assert_eq!(loaded.pair.light, "lattice-paper");
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn corrupt_appearance_uses_defaults_and_reports_diagnostic() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let path = AppearanceSettings::path_in(&home);
    std::fs::write(&path, "theme: [broken").unwrap();
    let (_home, settings, diagnostics) =
        crate::appearance::load_appearance_with_diagnostics().unwrap();
    assert_eq!(settings, AppearanceSettings::default());
    assert_eq!(diagnostics[0].code, "settings-invalid-yaml");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "theme: [broken");
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn user_theme_overrides_builtin_id() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let themes = ensure_user_themes_dir(&home).unwrap();
    let yaml = r##"
name: Custom Slate
id: lattice-slate
appearance: dark
palette:
  ground: "#111111"
  ground_raise: "#121212"
  panel: "#131313"
  slate: "#8ca2c4"
  text: "#e7ecf5"
  text_soft: "#b9c2d4"
  muted: "#8791a6"
  faint: "#5f6a80"
  accent: "#ff0000"
  accent_bright: "#ff8888"
  accent_deep: "#aa0000"
  danger: "#ff9d8a"
  ink_shadow: "#000000"
roles:
  bg: $ground
  bg_raise: $ground_raise
  panel: $panel
  slate: $slate
  text: $text
  text_soft: $text_soft
  muted: $muted
  faint: $faint
  accent: $accent
  accent_bright: $accent_bright
  accent_deep: $accent_deep
  danger: $danger
  shadow: $ink_shadow
fonts:
  display: Serif
  ui: Sans
  mono: Mono
shape:
  radius: 9px
  radius_sm: 6px
  radius_lg: 14px
  grid: 34px
  titlebar: 38px
  max_width: 1140px
"##;
    std::fs::write(themes.join("lattice-slate.theme.yaml"), yaml).unwrap();
    let (doc, _) = load_theme_by_id(&home, "lattice-slate").unwrap();
    assert_eq!(doc.name, "Custom Slate");
    let (list, diags) = discover_themes(&home).unwrap();
    assert!(diags.is_empty());
    let slate = list.iter().find(|t| t.id == "lattice-slate").unwrap();
    assert_eq!(slate.source, crate::ThemeSource::User);
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn workspace_accent_override() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(ws.join(".lattice")).unwrap();
    std::fs::write(
        WorkspaceThemeOverride::path_in(&ws),
        "accent: \"#00ff00\"\n",
    )
    .unwrap();
    let settings = AppearanceSettings::default();
    let resolved =
        resolve_active_theme(&home, &settings, SystemAppearance::Dark, Some(&ws)).unwrap();
    assert_eq!(
        resolved.vars.get("--lt-accent").map(String::as_str),
        Some("#00ff00")
    );
    let ov = load_workspace_override(&ws).unwrap();
    assert_eq!(ov.accent.as_deref(), Some("#00ff00"));
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn check_rejects_bad_ref() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.theme.yaml");
    std::fs::write(
        &path,
        r##"
name: Bad
id: bad
appearance: dark
palette:
  ground: "#000000"
roles:
  bg: $missing
  bg_raise: "#111111"
  panel: "#222222"
  slate: "#333333"
  text: "#ffffff"
  text_soft: "#eeeeee"
  muted: "#cccccc"
  faint: "#999999"
  accent: "#ff0000"
  accent_bright: "#ff8888"
  accent_deep: "#aa0000"
  danger: "#ff6666"
  shadow: "#000000"
fonts:
  display: Serif
  ui: Sans
  mono: Mono
shape:
  radius: 9px
  radius_sm: 6px
  radius_lg: 14px
  grid: 34px
  titlebar: 38px
  max_width: 1140px
"##,
    )
    .unwrap();
    let err = check_theme_file(&path).unwrap_err();
    assert!(err.to_string().contains("unknown palette ref") || err.to_string().contains("missing"));
}
