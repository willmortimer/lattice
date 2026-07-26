//! Parse and validate portable `*.deck/` presentation packages.
//!
//! A Deck keeps its semantic HTML slides, optional CSS theme, and Markdown
//! notes as ordinary inspectable files. This module intentionally owns only
//! canonical format validation; rendering and export live behind later
//! presentation adapters.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::is_safe_relative_path;

pub const DECK_FORMAT: &str = "lattice-deck";
pub const DECK_MANIFEST_FILENAME: &str = "deck.yaml";
pub const DECK_SUPPORTED_VERSION: u32 = 1;
pub const MAX_DECK_SLIDES: usize = 500;
const MAX_TRANSITION_DURATION_MS: u32 = 10_000;
const MAX_IDENTIFIER_LENGTH: usize = 128;
const MAX_TITLE_LENGTH: usize = 512;
const MAX_PRESENTATION_DURATION_MINUTES: u32 = 1_440;

#[derive(Debug, thiserror::Error)]
pub enum DeckError {
    #[error("invalid deck manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type DeckResult<T> = std::result::Result<T, DeckError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeckAspectRatio {
    #[serde(rename = "16:9")]
    Wide,
    #[serde(rename = "4:3")]
    Standard,
}

impl Default for DeckAspectRatio {
    fn default() -> Self {
        Self::Wide
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeckTransitionType {
    Cut,
    Fade,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeckTransitionDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckTransition {
    #[serde(rename = "type")]
    pub kind: DeckTransitionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<DeckTransitionDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckSlide {
    pub id: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<DeckTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeckTheme {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylesheet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeckPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default)]
    pub r#loop: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_minutes: Option<u32>,
}

/// The version-one `deck.yaml` model. Unknown keys are intentionally ignored
/// on parse so future authors can retain forward-compatible source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckManifest {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub aspect_ratio: DeckAspectRatio,
    #[serde(default)]
    pub theme: DeckTheme,
    pub slides: Vec<DeckSlide>,
    #[serde(default)]
    pub presentation: DeckPresentation,
}

impl DeckManifest {
    /// Load a package manifest and validate every referenced file stays within
    /// the canonical package directory, including through symlinks.
    pub fn load(package_or_manifest: &Path) -> DeckResult<Self> {
        let manifest_path = resolve_deck_manifest_path(package_or_manifest);
        let text = std::fs::read_to_string(&manifest_path).map_err(|source| DeckError::Io {
            path: manifest_path.clone(),
            source,
        })?;
        let manifest = Self::parse_str(&text, &manifest_path)?;
        manifest.check_package(&manifest_path)?;
        Ok(manifest)
    }

    /// Validate manifest syntax and in-document invariants. This is useful for
    /// proposal validation before package files have been materialized.
    pub fn parse_str(text: &str, path: &Path) -> DeckResult<Self> {
        let manifest: Self = serde_yaml::from_str(text).map_err(|source| DeckError::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    pub fn check_package(&self, manifest_path: &Path) -> DeckResult<()> {
        let package = manifest_path
            .parent()
            .ok_or_else(|| DeckError::InvalidManifest {
                path: manifest_path.to_path_buf(),
                message: "deck.yaml must have a package directory".into(),
            })?;
        let package = package.canonicalize().map_err(|source| DeckError::Io {
            path: package.to_path_buf(),
            source,
        })?;
        if !package.is_dir() {
            return Err(DeckError::InvalidManifest {
                path: manifest_path.to_path_buf(),
                message: "deck package must be a directory".into(),
            });
        }
        if let Some(stylesheet) = &self.theme.stylesheet {
            self.check_referenced_file(
                manifest_path,
                &package,
                stylesheet,
                "theme.stylesheet",
                is_css,
            )?;
        }
        for slide in &self.slides {
            self.check_referenced_file(
                manifest_path,
                &package,
                &slide.source,
                &format!("slide `{}` source", slide.id),
                is_html,
            )?;
            if let Some(notes) = &slide.notes {
                self.check_referenced_file(
                    manifest_path,
                    &package,
                    notes,
                    &format!("slide `{}` notes", slide.id),
                    is_markdown,
                )?;
            }
        }
        Ok(())
    }

    fn check(&self, path: &Path) -> DeckResult<()> {
        let invalid = |message: String| DeckError::InvalidManifest {
            path: path.to_path_buf(),
            message,
        };
        if self.format != DECK_FORMAT {
            return Err(invalid(format!(
                "expected format {DECK_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version != DECK_SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is not supported (expected {DECK_SUPPORTED_VERSION})",
                self.version
            )));
        }
        if !is_stable_id(&self.id) {
            return Err(invalid(
                "id must be a stable identifier: start with an ASCII letter or digit and contain only letters, digits, `_`, or `-`".into(),
            ));
        }
        if self.title.trim().is_empty() || self.title.chars().count() > MAX_TITLE_LENGTH {
            return Err(invalid(format!(
                "title must be non-empty and at most {MAX_TITLE_LENGTH} characters"
            )));
        }
        if self.slides.is_empty() {
            return Err(invalid("slides must contain at least one slide".into()));
        }
        if self.slides.len() > MAX_DECK_SLIDES {
            return Err(invalid(format!(
                "slides may contain at most {MAX_DECK_SLIDES} entries"
            )));
        }
        if let Some(stylesheet) = &self.theme.stylesheet {
            if !is_safe_relative_path(stylesheet) || !is_css(stylesheet) {
                return Err(invalid(
                    "theme.stylesheet must be a package-relative .css path".into(),
                ));
            }
        }
        let mut ids = BTreeSet::new();
        for slide in &self.slides {
            if !is_stable_id(&slide.id) {
                return Err(invalid(format!(
                    "slide id {:?} is not a stable identifier",
                    slide.id
                )));
            }
            if !ids.insert(&slide.id) {
                return Err(invalid(format!("slide id {:?} is duplicated", slide.id)));
            }
            if !is_safe_relative_path(&slide.source) || !is_html(&slide.source) {
                return Err(invalid(format!(
                    "slide `{}` source must be a package-relative .html path",
                    slide.id
                )));
            }
            if let Some(notes) = &slide.notes {
                if !is_safe_relative_path(notes) || !is_markdown(notes) {
                    return Err(invalid(format!(
                        "slide `{}` notes must be a package-relative Markdown path",
                        slide.id
                    )));
                }
            }
            if let Some(transition) = &slide.transition {
                validate_transition(transition).map_err(invalid)?;
            }
        }
        if let Some(start) = &self.presentation.start {
            if !ids.contains(start) {
                return Err(invalid(format!(
                    "presentation.start {:?} does not name a slide",
                    start
                )));
            }
        }
        if let Some(minutes) = self.presentation.duration_minutes {
            if !(1..=MAX_PRESENTATION_DURATION_MINUTES).contains(&minutes) {
                return Err(invalid(format!(
                    "presentation.duration_minutes must be 1..={MAX_PRESENTATION_DURATION_MINUTES}"
                )));
            }
        }
        Ok(())
    }

    fn check_referenced_file(
        &self,
        manifest_path: &Path,
        package: &Path,
        reference: &str,
        label: &str,
        extension_ok: fn(&str) -> bool,
    ) -> DeckResult<()> {
        let candidate = package.join(reference);
        if !candidate.is_file() {
            return Err(DeckError::InvalidManifest {
                path: manifest_path.to_path_buf(),
                message: format!("{label} {:?} does not exist as a file", reference),
            });
        }
        let resolved = candidate.canonicalize().map_err(|source| DeckError::Io {
            path: candidate.clone(),
            source,
        })?;
        if !resolved.starts_with(package) {
            return Err(DeckError::InvalidManifest {
                path: manifest_path.to_path_buf(),
                message: format!("{label} {:?} resolves outside the deck package", reference),
            });
        }
        if !extension_ok(reference) {
            return Err(DeckError::InvalidManifest {
                path: manifest_path.to_path_buf(),
                message: format!("{label} {:?} has an unsupported extension", reference),
            });
        }
        Ok(())
    }
}

pub fn resolve_deck_manifest_path(package_or_manifest: &Path) -> PathBuf {
    if package_or_manifest.is_file() {
        package_or_manifest.to_path_buf()
    } else {
        package_or_manifest.join(DECK_MANIFEST_FILENAME)
    }
}

fn is_stable_id(value: &str) -> bool {
    if value.is_empty() || value.chars().count() > MAX_IDENTIFIER_LENGTH {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_html(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("html" | "htm")
    )
}

fn is_css(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("css")
}

fn is_markdown(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("md" | "markdown")
    )
}

fn validate_transition(transition: &DeckTransition) -> Result<(), String> {
    if let Some(duration) = transition.duration_ms {
        if duration == 0 || duration > MAX_TRANSITION_DURATION_MS {
            return Err(format!(
                "transition duration_ms must be 1..={MAX_TRANSITION_DURATION_MS}"
            ));
        }
    }
    match transition.kind {
        DeckTransitionType::Push if transition.direction.is_none() => {
            Err("push transitions require direction".into())
        }
        DeckTransitionType::Cut | DeckTransitionType::Fade if transition.direction.is_some() => {
            Err("only push transitions may specify direction".into())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> &'static str {
        r#"
format: lattice-deck
version: 1
id: quarterly-review
title: Quarterly review
aspect_ratio: 16:9
theme:
  stylesheet: ./theme.css
slides:
  - id: title
    source: ./slides/001-title.html
    notes: ./notes/001-title.md
    transition:
      type: fade
      duration_ms: 280
  - id: metrics
    source: ./slides/002-metrics.html
    transition:
      type: push
      direction: left
presentation:
  start: title
  duration_minutes: 20
"#
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("slides")).unwrap();
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        std::fs::write(dir.path().join("theme.css"), "body {} ").unwrap();
        std::fs::write(dir.path().join("slides/001-title.html"), "<h1>Title</h1>").unwrap();
        std::fs::write(
            dir.path().join("slides/002-metrics.html"),
            "<h1>Metrics</h1>",
        )
        .unwrap();
        std::fs::write(dir.path().join("notes/001-title.md"), "# Notes").unwrap();
        std::fs::write(dir.path().join(DECK_MANIFEST_FILENAME), valid_manifest()).unwrap();
        dir
    }

    #[test]
    fn loads_complete_v1_package() {
        let dir = fixture();
        let manifest = DeckManifest::load(dir.path()).unwrap();
        assert_eq!(manifest.id, "quarterly-review");
        assert_eq!(manifest.slides.len(), 2);
        assert_eq!(manifest.aspect_ratio, DeckAspectRatio::Wide);
    }

    #[test]
    fn rejects_duplicate_and_invalid_ids() {
        let duplicate = valid_manifest().replace("id: metrics", "id: title");
        assert!(DeckManifest::parse_str(&duplicate, Path::new("deck.yaml"))
            .unwrap_err()
            .to_string()
            .contains("duplicated"));
        let invalid = valid_manifest().replace("id: quarterly-review", "id: no spaces");
        assert!(DeckManifest::parse_str(&invalid, Path::new("deck.yaml"))
            .unwrap_err()
            .to_string()
            .contains("stable identifier"));
    }

    #[test]
    fn rejects_bad_paths_and_unknown_start() {
        let traversal = valid_manifest().replace("./slides/001-title.html", "../escape.html");
        assert!(DeckManifest::parse_str(&traversal, Path::new("deck.yaml"))
            .unwrap_err()
            .to_string()
            .contains("package-relative"));
        let missing_start = valid_manifest().replace("start: title", "start: missing");
        assert!(
            DeckManifest::parse_str(&missing_start, Path::new("deck.yaml"))
                .unwrap_err()
                .to_string()
                .contains("does not name a slide")
        );
    }

    #[test]
    fn rejects_invalid_transition_and_slide_limit() {
        let bad_push = valid_manifest().replace("direction: left", "");
        assert!(DeckManifest::parse_str(&bad_push, Path::new("deck.yaml"))
            .unwrap_err()
            .to_string()
            .contains("require direction"));
        let mut yaml =
            String::from("format: lattice-deck\nversion: 1\nid: deck\ntitle: Deck\nslides:\n");
        for index in 0..=MAX_DECK_SLIDES {
            yaml.push_str(&format!(
                "  - id: s{index}\n    source: ./slides/{index}.html\n"
            ));
        }
        assert!(DeckManifest::parse_str(&yaml, Path::new("deck.yaml"))
            .unwrap_err()
            .to_string()
            .contains("at most"));
    }

    #[test]
    fn applies_defaults_and_preserves_forward_compatible_source() {
        let yaml = r#"
format: lattice-deck
version: 1
id: simple
title: Simple
slides:
  - id: title
    source: ./slides/title.html
future_renderer_hint: preserved-by-editors
"#;
        let manifest = DeckManifest::parse_str(yaml, Path::new("deck.yaml")).unwrap();
        assert_eq!(manifest.aspect_ratio, DeckAspectRatio::Wide);
        assert!(!manifest.presentation.r#loop);
        assert_eq!(manifest.presentation.start, None);
    }

    #[test]
    fn bounds_identity_title_and_timer_values() {
        let oversized_id =
            valid_manifest().replace("id: quarterly-review", &format!("id: {}", "a".repeat(129)));
        assert!(
            DeckManifest::parse_str(&oversized_id, Path::new("deck.yaml"))
                .unwrap_err()
                .to_string()
                .contains("stable identifier")
        );
        let oversized_title = valid_manifest().replace(
            "title: Quarterly review",
            &format!("title: {}", "a".repeat(513)),
        );
        assert!(
            DeckManifest::parse_str(&oversized_title, Path::new("deck.yaml"))
                .unwrap_err()
                .to_string()
                .contains("at most")
        );
        let bad_timer = valid_manifest().replace("duration_minutes: 20", "duration_minutes: 0");
        assert!(DeckManifest::parse_str(&bad_timer, Path::new("deck.yaml"))
            .unwrap_err()
            .to_string()
            .contains("duration_minutes"));
    }

    #[test]
    fn load_requires_existing_contained_files() {
        let dir = fixture();
        std::fs::remove_file(dir.path().join("slides/002-metrics.html")).unwrap();
        assert!(DeckManifest::load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("does not exist"));
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_symlink_escape() {
        let dir = fixture();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("outside.html");
        std::fs::write(&outside_file, "<h1>outside</h1>").unwrap();
        std::fs::remove_file(dir.path().join("slides/002-metrics.html")).unwrap();
        std::os::unix::fs::symlink(&outside_file, dir.path().join("slides/002-metrics.html"))
            .unwrap();
        assert!(DeckManifest::load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("outside the deck package"));
    }
}
