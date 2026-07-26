use std::sync::Mutex;

use tempfile::tempdir;

use crate::appearance::{
    ensure_user_themes_dir, save_appearance, AppearanceMode, AppearanceSettings,
};
use crate::builtin::load_builtin;
use crate::discover::{check_theme_file, discover_themes, load_theme_by_id};
use crate::document::builtin_path;
use crate::flatten::flatten_theme;
use crate::font_pack::{
    discover_font_packs, load_builtin_font_pack, load_font_pack_by_id, resolve_font_pack_id,
    BUILTIN_FONT_PACK_IDS, DEFAULT_FONT_PACK_ID, FONT_PACK_FOLLOW_THEME,
};
use crate::override_file::{load_workspace_override, WorkspaceThemeOverride};
use crate::resolve::{resolve_active_theme, SystemAppearance};
use lattice_core::ensure_lattice_home;

/// `LATTICE_HOME` is process-global; serialize tests that mutate it.
static HOME_LOCK: Mutex<()> = Mutex::new(());

fn flatten_builtin(id: &str) -> std::collections::BTreeMap<String, String> {
    let doc = load_builtin(id).unwrap();
    let pack = load_builtin_font_pack(&doc.font_pack).unwrap();
    flatten_theme(&doc, &pack.fonts, &builtin_path(id)).unwrap()
}

#[test]
fn builtins_parse_and_flatten() {
    for id in crate::builtin::BUILTIN_IDS {
        let vars = flatten_builtin(id);
        assert!(vars.contains_key("--lt-bg"));
        assert!(vars.contains_key("--lt-accent"));
        assert!(vars.contains_key("--lt-font-ui"));
        assert!(vars["--lt-accent-wash"].contains("color-mix"));
    }
}

#[test]
fn builtin_font_packs_parse() {
    for id in BUILTIN_FONT_PACK_IDS {
        let pack = load_builtin_font_pack(id).unwrap();
        assert_eq!(pack.id, *id);
        assert!(!pack.fonts.display.is_empty());
        assert!(!pack.fonts.ui.is_empty());
        assert!(!pack.fonts.mono.is_empty());
    }
}

#[test]
fn cupertino_defaults_to_apple_font_pack() {
    let doc = load_builtin("cupertino").unwrap();
    assert_eq!(doc.font_pack, "apple");
    let pack = load_builtin_font_pack(&doc.font_pack).unwrap();
    assert!(pack.fonts.ui.contains("SF Pro Text") || pack.fonts.ui.contains("-apple-system"));
}

#[test]
fn slate_defaults_to_lattice_font_pack() {
    let doc = load_builtin("lattice-slate").unwrap();
    assert_eq!(doc.font_pack, "lattice");
    let vars = flatten_builtin("lattice-slate");
    assert!(vars["--lt-font-display"].contains("Fraunces"));
    assert!(vars["--lt-font-mono"].contains("JetBrains Mono"));
}

#[test]
fn resolve_font_pack_id_follows_theme_or_override() {
    assert_eq!(
        resolve_font_pack_id(FONT_PACK_FOLLOW_THEME, "apple"),
        "apple"
    );
    assert_eq!(resolve_font_pack_id("signal", "apple"), "signal");
    assert_eq!(
        resolve_font_pack_id("", ""),
        DEFAULT_FONT_PACK_ID
    );
}

#[test]
fn appearance_font_pack_override_wins() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let settings = AppearanceSettings {
        theme: "lattice-slate".into(),
        font_pack: "atelier".into(),
        ..Default::default()
    };
    let resolved =
        resolve_active_theme(&home, &settings, SystemAppearance::Dark, None).unwrap();
    assert_eq!(resolved.font_pack, "atelier");
    assert!(resolved.vars["--lt-font-display"].contains("Literata"));
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn unknown_font_pack_falls_back_to_lattice() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let settings = AppearanceSettings {
        font_pack: "does-not-exist".into(),
        ..Default::default()
    };
    let resolved =
        resolve_active_theme(&home, &settings, SystemAppearance::Dark, None).unwrap();
    assert_eq!(resolved.font_pack, "lattice");
    assert!(!resolved.diagnostics.is_empty());
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn discover_font_packs_lists_builtins() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let (packs, diags) = discover_font_packs(&home).unwrap();
    assert!(diags.is_empty());
    assert!(packs.iter().any(|p| p.id == "lattice"));
    assert!(packs.iter().any(|p| p.id == "apple"));
    assert!(packs.iter().any(|p| p.id == "atelier"));
    assert!(packs.iter().any(|p| p.id == "signal"));
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn load_font_pack_by_id_user_override() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let packs_dir = home.settings.join("font-packs");
    std::fs::create_dir_all(&packs_dir).unwrap();
    std::fs::write(
        packs_dir.join("lattice.font-pack.yaml"),
        r##"
name: Custom Lattice
id: lattice
fonts:
  display: CustomDisplay
  ui: CustomUi
  mono: CustomMono
"##,
    )
    .unwrap();
    let (doc, _) = load_font_pack_by_id(&home, "lattice").unwrap();
    assert_eq!(doc.name, "Custom Lattice");
    assert_eq!(doc.fonts.ui, "CustomUi");
    std::env::remove_var("LATTICE_HOME");
}

#[test]
fn terminal_palettes_flatten_ansi_palette() {
    // Terminal-derived themes must carry their explicit ANSI palette through
    // flatten as --lt-term-* vars.
    for id in [
        "catppuccin-mocha",
        "nord",
        "github-dark",
        "dracula",
        "solarized-dark",
        "tokyo-night",
        "gruvbox-dark",
        "one-dark",
        "rose-pine-moon",
        "kanagawa-wave",
    ] {
        let vars = flatten_builtin(id);
        for key in crate::document::TERMINAL_ANSI_KEYS {
            let var = format!("--lt-term-{}", key.replace('_', "-"));
            assert!(vars.contains_key(&var), "{id} missing {var}");
        }
    }
    // Dracula ANSI green is canonical, not role-derived.
    let vars = flatten_builtin("dracula");
    assert_eq!(
        vars.get("--lt-term-green").map(String::as_str),
        Some("#50fa7b")
    );
}

#[test]
fn themes_without_terminal_block_emit_no_term_vars() {
    let vars = flatten_builtin("lattice-slate");
    assert!(!vars.keys().any(|k| k.starts_with("--lt-term-")));
}

#[test]
fn terminal_block_requires_all_ansi_slots() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("partial.theme.yaml");
    let yaml = r##"
name: Partial
id: partial
appearance: dark
palette:
  ground: "#000000"
roles:
  bg: $ground
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
terminal:
  black: "#000000"
  red: "#ff0000"
font_pack: lattice
shape:
  radius: 9px
  radius_sm: 6px
  radius_lg: 14px
  grid: 34px
  titlebar: 38px
  max_width: 1140px
"##;
    std::fs::write(&path, yaml).unwrap();
    let err = check_theme_file(&path).unwrap_err();
    assert!(err
        .to_string()
        .contains("terminal missing required ANSI key"));
}

#[test]
fn oled_ground_is_true_black() {
    let vars = flatten_builtin("lattice-oled");
    assert_eq!(vars.get("--lt-bg").map(String::as_str), Some("#000000"));
}

#[test]
fn slate_ground_matches_shipped_default() {
    let vars = flatten_builtin("lattice-slate");
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
        font_pack: "signal".into(),
        ..Default::default()
    };
    save_appearance(&settings).unwrap();
    let (_home, loaded) = crate::appearance::load_appearance().unwrap();
    assert_eq!(loaded.mode, AppearanceMode::Auto);
    assert_eq!(loaded.pair.light, "lattice-paper");
    assert_eq!(loaded.font_pack, "signal");
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
fn legacy_appearance_is_upgraded_once_without_a_persistent_warning() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    std::env::set_var("LATTICE_HOME", dir.path());
    let home = ensure_lattice_home().unwrap();
    let path = AppearanceSettings::path_in(&home);
    std::fs::write(&path, "theme: lattice-paper\nmode: fixed\n").unwrap();

    let (_home, settings, diagnostics) =
        crate::appearance::load_appearance_with_diagnostics().unwrap();

    assert_eq!(settings.theme, "lattice-paper");
    assert_eq!(settings.font_pack, FONT_PACK_FOLLOW_THEME);
    assert!(diagnostics.is_empty());
    let upgraded = std::fs::read_to_string(path).unwrap();
    assert!(upgraded.contains("format: lattice-appearance-settings"));
    assert!(upgraded.contains("version: 1"));
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
font_pack: lattice
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
font_pack: lattice
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
