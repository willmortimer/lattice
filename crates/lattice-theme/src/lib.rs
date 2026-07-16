//! Theme documents, appearance settings, and CSS-variable resolution.
//!
//! Themes are plain YAML files (same ethos as `lattice.yaml`). Components
//! never see theme ids — they consume `--lt-*` CSS custom properties only.
//! This crate parses, validates, discovers, and flattens themes for the
//! desktop shell, CLI (`lattice theme check`), and settings persistence.

mod appearance;
mod builtin;
mod discover;
mod document;
mod error;
mod flatten;
mod override_file;
mod resolve;

pub use appearance::{
    load_appearance, load_appearance_with_diagnostics, save_appearance, AppearanceMode,
    AppearanceSettings, ThemePair, APPEARANCE_FILENAME,
};
pub use discover::{
    check_theme_file, discover_themes, load_theme_by_id, theme_dirs, ThemeDiagnostic, ThemeSource,
    ThemeSummary,
};
pub use document::{Appearance, ThemeDocument, THEME_FILE_SUFFIX};
pub use error::Error;
pub use flatten::{apply_accent_override, flatten_theme};
pub use override_file::{
    load_workspace_override, WorkspaceThemeOverride, WORKSPACE_THEME_FILENAME,
};
pub use resolve::{resolve_active_theme, ResolvedTheme, SystemAppearance};

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
