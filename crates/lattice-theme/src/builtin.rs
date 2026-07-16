//! Built-in themes embedded in the binary.

use crate::document::{builtin_path, ThemeDocument};
use crate::{Error, Result};

const SLATE_YAML: &str = include_str!("../../../themes/lattice-slate.theme.yaml");
const PAPER_YAML: &str = include_str!("../../../themes/lattice-paper.theme.yaml");
const CARBON_YAML: &str = include_str!("../../../themes/lattice-carbon.theme.yaml");
const FJORD_YAML: &str = include_str!("../../../themes/lattice-fjord.theme.yaml");
const ULTRAVIOLET_YAML: &str = include_str!("../../../themes/lattice-ultraviolet.theme.yaml");
const BLUEPRINT_YAML: &str = include_str!("../../../themes/lattice-blueprint.theme.yaml");
const VELLUM_YAML: &str = include_str!("../../../themes/lattice-vellum.theme.yaml");

/// Ids of themes shipped with Lattice.
pub const BUILTIN_IDS: &[&str] = &[
    "lattice-slate",
    "lattice-paper",
    "lattice-carbon",
    "lattice-fjord",
    "lattice-ultraviolet",
    "lattice-blueprint",
    "lattice-vellum",
];

/// Load a built-in theme by id.
pub fn load_builtin(id: &str) -> Result<ThemeDocument> {
    let (yaml, path) = match id {
        "lattice-slate" => (SLATE_YAML, builtin_path("lattice-slate")),
        "lattice-paper" => (PAPER_YAML, builtin_path("lattice-paper")),
        "lattice-carbon" => (CARBON_YAML, builtin_path("lattice-carbon")),
        "lattice-fjord" => (FJORD_YAML, builtin_path("lattice-fjord")),
        "lattice-ultraviolet" => (ULTRAVIOLET_YAML, builtin_path("lattice-ultraviolet")),
        "lattice-blueprint" => (BLUEPRINT_YAML, builtin_path("lattice-blueprint")),
        "lattice-vellum" => (VELLUM_YAML, builtin_path("lattice-vellum")),
        _ => return Err(Error::ThemeNotFound(id.to_string())),
    };
    ThemeDocument::parse(&path, yaml)
}
