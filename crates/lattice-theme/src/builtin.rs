//! Built-in themes embedded in the binary.

use crate::document::{builtin_path, ThemeDocument};
use crate::{Error, Result};

/// Id → embedded YAML for every theme shipped with Lattice.
const BUILTINS: &[(&str, &str)] = &[
    // Lattice originals
    (
        "lattice-slate",
        include_str!("../../../themes/lattice-slate.theme.yaml"),
    ),
    (
        "lattice-paper",
        include_str!("../../../themes/lattice-paper.theme.yaml"),
    ),
    (
        "lattice-carbon",
        include_str!("../../../themes/lattice-carbon.theme.yaml"),
    ),
    (
        "lattice-fjord",
        include_str!("../../../themes/lattice-fjord.theme.yaml"),
    ),
    (
        "lattice-ultraviolet",
        include_str!("../../../themes/lattice-ultraviolet.theme.yaml"),
    ),
    (
        "lattice-blueprint",
        include_str!("../../../themes/lattice-blueprint.theme.yaml"),
    ),
    (
        "lattice-vellum",
        include_str!("../../../themes/lattice-vellum.theme.yaml"),
    ),
    (
        "lattice-ember",
        include_str!("../../../themes/lattice-ember.theme.yaml"),
    ),
    (
        "lattice-moss",
        include_str!("../../../themes/lattice-moss.theme.yaml"),
    ),
    (
        "lattice-midnight",
        include_str!("../../../themes/lattice-midnight.theme.yaml"),
    ),
    (
        "lattice-copper",
        include_str!("../../../themes/lattice-copper.theme.yaml"),
    ),
    (
        "lattice-rosewood",
        include_str!("../../../themes/lattice-rosewood.theme.yaml"),
    ),
    (
        "lattice-graphite",
        include_str!("../../../themes/lattice-graphite.theme.yaml"),
    ),
    (
        "lattice-glacier",
        include_str!("../../../themes/lattice-glacier.theme.yaml"),
    ),
    (
        "lattice-sandstone",
        include_str!("../../../themes/lattice-sandstone.theme.yaml"),
    ),
    (
        "lattice-orchid",
        include_str!("../../../themes/lattice-orchid.theme.yaml"),
    ),
    (
        "lattice-meadow",
        include_str!("../../../themes/lattice-meadow.theme.yaml"),
    ),
    // Platform looks
    (
        "cupertino",
        include_str!("../../../themes/cupertino.theme.yaml"),
    ),
    (
        "lattice-oled",
        include_str!("../../../themes/lattice-oled.theme.yaml"),
    ),
    // Adopted terminal standards (carry a `terminal:` ANSI palette)
    (
        "catppuccin-mocha",
        include_str!("../../../themes/catppuccin-mocha.theme.yaml"),
    ),
    ("nord", include_str!("../../../themes/nord.theme.yaml")),
    (
        "github-dark",
        include_str!("../../../themes/github-dark.theme.yaml"),
    ),
    (
        "dracula",
        include_str!("../../../themes/dracula.theme.yaml"),
    ),
    (
        "solarized-dark",
        include_str!("../../../themes/solarized-dark.theme.yaml"),
    ),
];

/// Ids of themes shipped with Lattice.
pub const BUILTIN_IDS: &[&str] = &[
    "lattice-slate",
    "lattice-paper",
    "lattice-carbon",
    "lattice-fjord",
    "lattice-ultraviolet",
    "lattice-blueprint",
    "lattice-vellum",
    "lattice-ember",
    "lattice-moss",
    "lattice-midnight",
    "lattice-copper",
    "lattice-rosewood",
    "lattice-graphite",
    "lattice-glacier",
    "lattice-sandstone",
    "lattice-orchid",
    "lattice-meadow",
    "cupertino",
    "lattice-oled",
    "catppuccin-mocha",
    "nord",
    "github-dark",
    "dracula",
    "solarized-dark",
];

/// Load a built-in theme by id.
pub fn load_builtin(id: &str) -> Result<ThemeDocument> {
    let yaml = BUILTINS
        .iter()
        .find(|(builtin_id, _)| *builtin_id == id)
        .map(|(_, yaml)| *yaml)
        .ok_or_else(|| Error::ThemeNotFound(id.to_string()))?;
    ThemeDocument::parse(&builtin_path(id), yaml)
}
