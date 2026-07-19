//! Flatten a theme document into `--lt-*` / `--l-*` CSS custom properties.
//!
//! Derived washes use the same `color-mix(in oklch, …)` recipes as
//! `scripts/compile-theme.mjs` so build-time and runtime stay in lockstep.

use std::collections::BTreeMap;
use std::path::Path;

use crate::document::ThemeDocument;
use crate::Result;

/// Flatten `theme` to a map of CSS custom property name → value.
pub fn flatten_theme(theme: &ThemeDocument, path: &Path) -> Result<BTreeMap<String, String>> {
    let roles = theme.resolved_roles(path)?;
    let role = |k: &str| -> Result<String> {
        roles.get(k).cloned().ok_or_else(|| {
            crate::Error::invalid(path, format!("required role {k} missing after validation"))
        })
    };

    let on_accent = roles
        .get("on_accent")
        .cloned()
        .or_else(|| theme.palette.get("on_accent").cloned())
        .unwrap_or_else(|| "#201403".into());

    let mut vars = BTreeMap::new();

    // Roles
    vars.insert("--lt-bg".into(), role("bg")?);
    vars.insert("--lt-bg-raise".into(), role("bg_raise")?);
    vars.insert("--lt-panel".into(), role("panel")?);
    vars.insert("--lt-slate".into(), role("slate")?);
    vars.insert("--lt-text".into(), role("text")?);
    vars.insert("--lt-text-soft".into(), role("text_soft")?);
    vars.insert("--lt-muted".into(), role("muted")?);
    vars.insert("--lt-faint".into(), role("faint")?);
    vars.insert("--lt-accent".into(), role("accent")?);
    vars.insert("--lt-accent-bright".into(), role("accent_bright")?);
    vars.insert("--lt-accent-deep".into(), role("accent_deep")?);
    vars.insert("--lt-danger".into(), role("danger")?);
    vars.insert("--lt-shadow".into(), role("shadow")?);
    vars.insert("--lt-on-accent".into(), on_accent);
    vars.insert("--lt-surface".into(), "var(--lt-panel)".into());

    // Derived
    vars.insert(
        "--lt-hover".into(),
        "color-mix(in oklch, var(--lt-slate) 7%, transparent)".into(),
    );
    vars.insert(
        "--lt-line".into(),
        "color-mix(in oklch, var(--lt-slate) 12%, transparent)".into(),
    );
    vars.insert(
        "--lt-line-strong".into(),
        "color-mix(in oklch, var(--lt-slate) 22%, transparent)".into(),
    );
    vars.insert(
        "--lt-border".into(),
        "color-mix(in oklch, var(--lt-slate) 18%, transparent)".into(),
    );
    vars.insert(
        "--lt-accent-wash".into(),
        "color-mix(in oklch, var(--lt-accent) 10%, transparent)".into(),
    );
    vars.insert(
        "--lt-accent-glow".into(),
        "color-mix(in oklch, var(--lt-accent) 55%, transparent)".into(),
    );
    vars.insert(
        "--lt-accent-glow-soft".into(),
        "color-mix(in oklch, var(--lt-accent) 35%, transparent)".into(),
    );
    vars.insert(
        "--lt-accent-glow-mid".into(),
        "color-mix(in oklch, var(--lt-accent) 22%, transparent)".into(),
    );
    vars.insert(
        "--lt-accent-glow-strong".into(),
        "color-mix(in oklch, var(--lt-accent) 45%, transparent)".into(),
    );
    vars.insert(
        "--lt-accent-underline".into(),
        "color-mix(in oklch, var(--lt-accent) 35%, transparent)".into(),
    );
    vars.insert(
        "--lt-node-dot".into(),
        "color-mix(in oklch, var(--lt-slate) 26%, transparent)".into(),
    );
    vars.insert(
        "--lt-node-dot-soft".into(),
        "color-mix(in oklch, var(--lt-slate) 20%, transparent)".into(),
    );
    vars.insert(
        "--lt-scrim".into(),
        "color-mix(in oklch, var(--lt-bg) 60%, transparent)".into(),
    );
    vars.insert(
        "--lt-scrim-deep".into(),
        "color-mix(in oklch, var(--lt-bg) 72%, #06080c)".into(),
    );
    vars.insert(
        "--lt-shadow-md".into(),
        "color-mix(in oklch, var(--lt-shadow) 35%, transparent)".into(),
    );
    vars.insert(
        "--lt-shadow-lg".into(),
        "color-mix(in oklch, var(--lt-shadow) 45%, transparent)".into(),
    );

    // Terminal (optional ANSI palette from the theme's `terminal:` block).
    if let Some(terminal) = theme.resolved_terminal(path)? {
        for (key, value) in terminal {
            vars.insert(format!("--lt-term-{}", key.replace('_', "-")), value);
        }
    }

    // Fonts / shape
    vars.insert("--lt-font-display".into(), theme.fonts.display.clone());
    vars.insert("--lt-font-ui".into(), theme.fonts.ui.clone());
    vars.insert("--lt-font-mono".into(), theme.fonts.mono.clone());
    vars.insert("--lt-radius".into(), theme.shape.radius.clone());
    vars.insert("--lt-radius-sm".into(), theme.shape.radius_sm.clone());
    vars.insert("--lt-radius-lg".into(), theme.shape.radius_lg.clone());
    vars.insert("--lt-grid".into(), theme.shape.grid.clone());
    vars.insert("--lt-titlebar".into(), theme.shape.titlebar.clone());
    vars.insert("--lt-max-width".into(), theme.shape.max_width.clone());

    // Site aliases
    vars.insert("--l-bg".into(), "var(--lt-bg)".into());
    vars.insert("--l-bg-2".into(), "var(--lt-bg-raise)".into());
    vars.insert("--l-panel".into(), "var(--lt-panel)".into());
    vars.insert("--l-panel-2".into(), "var(--lt-panel)".into());
    vars.insert("--l-line".into(), "var(--lt-line)".into());
    vars.insert("--l-line-strong".into(), "var(--lt-line-strong)".into());
    vars.insert("--l-border".into(), "var(--lt-border)".into());
    vars.insert("--l-text".into(), "var(--lt-text)".into());
    vars.insert("--l-text-soft".into(), "var(--lt-text-soft)".into());
    vars.insert("--l-muted".into(), "var(--lt-muted)".into());
    vars.insert("--l-faint".into(), "var(--lt-faint)".into());
    vars.insert("--l-amber".into(), "var(--lt-accent)".into());
    vars.insert("--l-amber-bright".into(), "var(--lt-accent-bright)".into());
    vars.insert("--l-amber-deep".into(), "var(--lt-accent-deep)".into());
    vars.insert("--l-amber-glow".into(), "var(--lt-accent-glow)".into());
    vars.insert("--l-amber-wash".into(), "var(--lt-accent-wash)".into());
    vars.insert("--l-font-display".into(), "var(--lt-font-display)".into());
    vars.insert("--l-font-body".into(), "var(--lt-font-ui)".into());
    vars.insert("--l-font-mono".into(), "var(--lt-font-mono)".into());
    vars.insert("--l-maxw".into(), "var(--lt-max-width)".into());
    vars.insert("--l-radius".into(), "var(--lt-radius-lg)".into());
    vars.insert("--l-radius-sm".into(), "var(--lt-radius)".into());
    vars.insert("--l-grid-size".into(), "var(--lt-grid)".into());

    Ok(vars)
}

/// Apply an accent override onto an already-flattened var map (workspace knob).
pub fn apply_accent_override(vars: &mut BTreeMap<String, String>, accent: &str) {
    vars.insert("--lt-accent".into(), accent.to_string());
    // Bright/deep stay theme-relative; washes already reference var(--lt-accent).
}
