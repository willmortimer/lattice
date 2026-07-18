//! Workspace ignore patterns from `lattice.yaml`.
//!
//! Patterns use gitignore semantics (via the `ignore` crate): trailing `/`
//! for directories, `*` / `**`, and `!` negation. They apply on top of the
//! hard exclusions (hidden entries, `.lattice/`, editor noise).

use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::manifest::{WorkspaceManifest, WORKSPACE_MANIFEST_FILENAME};
use crate::{Error, Result};

/// Compiled ignore matcher for one workspace root.
#[derive(Debug, Clone)]
pub struct WorkspaceIgnore {
    matcher: Gitignore,
}

impl WorkspaceIgnore {
    /// Matcher that never ignores paths.
    pub fn empty() -> Self {
        Self {
            matcher: Gitignore::empty(),
        }
    }

    /// Build a matcher for `patterns` relative to `root`.
    ///
    /// Empty / whitespace-only lines are skipped. Invalid globs fail with
    /// [`Error::InvalidManifest`].
    pub fn from_patterns(root: &Path, patterns: &[String]) -> Result<Self> {
        if patterns.is_empty() {
            return Ok(Self::empty());
        }

        let mut builder = GitignoreBuilder::new(root);
        for (index, raw) in patterns.iter().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            builder
                .add_line(None, line)
                .map_err(|source| Error::InvalidManifest {
                    path: root.join(WORKSPACE_MANIFEST_FILENAME),
                    message: format!("ignore[{index}]: invalid pattern {line:?}: {source}"),
                })?;
        }

        let matcher = builder.build().map_err(|source| Error::InvalidManifest {
            path: root.join(WORKSPACE_MANIFEST_FILENAME),
            message: format!("failed to compile ignore patterns: {source}"),
        })?;
        Ok(Self { matcher })
    }

    /// Compile patterns from a parsed manifest.
    pub fn from_manifest(root: &Path, manifest: &WorkspaceManifest) -> Result<Self> {
        Self::from_patterns(root, &manifest.ignore)
    }

    /// Load `lattice.yaml` at `root` and compile its `ignore:` list.
    ///
    /// Returns [`Self::empty`] when no manifest exists (e.g. watcher tests).
    pub fn for_workspace_root(root: &Path) -> Result<Self> {
        let manifest_path = root.join(WORKSPACE_MANIFEST_FILENAME);
        if !manifest_path.exists() {
            return Ok(Self::empty());
        }
        let manifest = WorkspaceManifest::load(&manifest_path)?;
        Self::from_manifest(root, &manifest)
    }

    /// Whether `rel` (workspace-relative) should be excluded from scan,
    /// watch, and index.
    ///
    /// When `is_dir` is unknown, both file and directory interpretations are
    /// tried so directory-only patterns (`node_modules/`) still match.
    pub fn is_ignored(&self, rel: &Path, is_dir: Option<bool>) -> bool {
        match is_dir {
            Some(is_dir) => self
                .matcher
                .matched_path_or_any_parents(rel, is_dir)
                .is_ignore(),
            None => {
                self.matcher
                    .matched_path_or_any_parents(rel, false)
                    .is_ignore()
                    || self
                        .matcher
                        .matched_path_or_any_parents(rel, true)
                        .is_ignore()
            }
        }
    }
}

/// Normalize a workspace-relative path for ignore matching (forward slashes).
pub(crate) fn path_for_match(rel: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(rel.to_string_lossy().replace('\\', "/"))
    } else {
        rel.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_patterns_never_match() {
        let ignore = WorkspaceIgnore::empty();
        assert!(!ignore.is_ignored(Path::new("Notes/a.md"), Some(false)));
        assert!(!ignore.is_ignored(Path::new("node_modules"), Some(true)));
    }

    #[test]
    fn directory_pattern_skips_children() {
        let root = Path::new("/workspace");
        let ignore =
            WorkspaceIgnore::from_patterns(root, &["node_modules/".into(), "*.log".into()])
                .unwrap();
        assert!(ignore.is_ignored(Path::new("node_modules"), Some(true)));
        assert!(ignore.is_ignored(Path::new("node_modules/leftpad/index.js"), Some(false)));
        assert!(ignore.is_ignored(Path::new("debug.log"), Some(false)));
        assert!(!ignore.is_ignored(Path::new("Notes/Ideas.md"), Some(false)));
    }

    #[test]
    fn negation_reincludes_path() {
        let ignore = WorkspaceIgnore::from_patterns(
            Path::new("/workspace"),
            &["*.log".into(), "!important.log".into()],
        )
        .unwrap();
        assert!(ignore.is_ignored(Path::new("debug.log"), Some(false)));
        assert!(!ignore.is_ignored(Path::new("important.log"), Some(false)));
    }

    #[test]
    fn whitespace_only_patterns_are_skipped() {
        let ignore = WorkspaceIgnore::from_patterns(
            Path::new("/workspace"),
            &["  ".into(), "\t".into(), "node_modules/".into()],
        )
        .unwrap();
        assert!(ignore.is_ignored(Path::new("node_modules"), Some(true)));
    }

    #[test]
    fn unusual_but_valid_gitignore_patterns_compile() {
        // gitignore / globset treat many odd strings as literals; Lattice
        // follows that leniency so workspace open does not reject patterns
        // that `git check-ignore` would accept.
        let ignore =
            WorkspaceIgnore::from_patterns(Path::new("/workspace"), &["[".into()]).unwrap();
        assert!(ignore.is_ignored(Path::new("["), Some(false)));
    }
}
