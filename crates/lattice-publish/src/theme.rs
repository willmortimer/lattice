//! Inline `--lt-*` theme tokens for offline HTML shells.

use std::collections::BTreeMap;

use lattice_theme::{flatten_theme, load_builtin};

use crate::error::{Error, Result};

const DEFAULT_THEME_ID: &str = "lattice-slate";

/// Flatten a built-in theme into CSS custom properties for `:root`.
pub fn builtin_theme_vars(theme_id: Option<&str>) -> Result<BTreeMap<String, String>> {
    let id = theme_id.unwrap_or(DEFAULT_THEME_ID);
    let doc = load_builtin(id)?;
    let path_buf = std::path::PathBuf::from(format!("builtin:{id}.theme.yaml"));
    flatten_theme(&doc, &path_buf).map_err(Error::from)
}

/// Render a `:root { … }` block from theme variables.
pub fn theme_css(vars: &BTreeMap<String, String>) -> String {
    let mut css = String::from(":root {\n");
    for (key, value) in vars {
        css.push_str("  ");
        css.push_str(key);
        css.push_str(": ");
        css.push_str(value);
        css.push_str(";\n");
    }
    css.push_str("}\n");
    css
}

/// Compact HTML style block with theme tokens plus publish shell basics.
pub fn shell_style_block(vars: &BTreeMap<String, String>) -> String {
    format!(
        r#"<style>
{theme}
html, body {{
  margin: 0;
  padding: 0;
  background: var(--lt-bg, #f4f6fa);
  color: var(--lt-text, #121822);
  font-family: var(--lt-font-ui, system-ui, -apple-system, sans-serif);
  line-height: 1.5;
}}
main {{
  max-width: 52rem;
  margin: 0 auto;
  padding: 1.5rem 1.25rem 3rem;
}}
.lt-page h1, .lt-page h2, .lt-page h3, .lt-page h4 {{
  font-family: var(--lt-font-display, Georgia, "Times New Roman", serif);
  font-weight: 600;
  line-height: 1.25;
  color: var(--lt-text, #121822);
}}
.lt-page a {{ color: var(--lt-accent, #c47a0a); }}
.lt-page pre, .lt-page code {{
  font-family: var(--lt-font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
  background: var(--lt-bg-raise, #eef1f7);
}}
.lt-page pre {{
  padding: 0.85rem 1rem;
  overflow-x: auto;
  border-radius: var(--lt-radius, 9px);
  border: 1px solid var(--lt-border, #c5ccd8);
}}
.lt-page code {{ padding: 0.1em 0.35em; border-radius: 4px; }}
.lt-page pre code {{ padding: 0; background: transparent; }}
.lt-page blockquote {{
  margin: 1rem 0;
  padding-left: 1rem;
  border-left: 3px solid var(--lt-accent, #c47a0a);
  color: var(--lt-text-soft, #3a4558);
}}
.lt-banner {{
  margin: 0 0 1.25rem;
  padding: 0.55rem 0.75rem;
  border: 1px solid var(--lt-border, #c5ccd8);
  border-radius: var(--lt-radius-sm, 6px);
  background: var(--lt-bg-raise, #eef1f7);
  color: var(--lt-muted, #6b778c);
  font-size: 0.85rem;
}}
.lt-grid {{
  display: grid;
  gap: 1rem;
  grid-template-columns: repeat(12, minmax(0, 1fr));
}}
.lt-card {{
  border: 1px solid var(--lt-border, #c5ccd8);
  border-radius: var(--lt-radius, 9px);
  background: var(--lt-panel, #fff);
  padding: 0.9rem 1rem;
}}
.lt-card h2 {{
  margin: 0 0 0.65rem;
  font-size: 0.95rem;
  font-weight: 600;
}}
.lt-metric {{
  font-size: 1.85rem;
  font-weight: 600;
  color: var(--lt-accent, #c47a0a);
}}
.lt-muted {{ color: var(--lt-muted, #6b778c); font-size: 0.85rem; }}
.lt-table-wrap {{ overflow-x: auto; }}
table.lt-table {{
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}}
table.lt-table th, table.lt-table td {{
  border-bottom: 1px solid var(--lt-border, #c5ccd8);
  padding: 0.4rem 0.5rem;
  text-align: left;
  vertical-align: top;
}}
table.lt-table th {{
  color: var(--lt-muted, #6b778c);
  font-weight: 600;
}}
</style>"#,
        theme = theme_css(vars)
    )
}
