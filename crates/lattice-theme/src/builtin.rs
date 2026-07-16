//! Built-in themes embedded in the binary.

use crate::document::{builtin_path, ThemeDocument};
use crate::{Error, Result};

const SLATE_YAML: &str = include_str!("../../../themes/lattice-slate.theme.yaml");
const PAPER_YAML: &str = include_str!("../../../themes/lattice-paper.theme.yaml");

/// Ids of themes shipped with Lattice.
pub const BUILTIN_IDS: &[&str] = &["lattice-slate", "lattice-paper"];

/// Load a built-in theme by id.
pub fn load_builtin(id: &str) -> Result<ThemeDocument> {
    let (yaml, path) = match id {
        "lattice-slate" => (SLATE_YAML, builtin_path("lattice-slate")),
        "lattice-paper" => (PAPER_YAML, builtin_path("lattice-paper")),
        _ => return Err(Error::ThemeNotFound(id.to_string())),
    };
    ThemeDocument::parse(&path, yaml)
}
