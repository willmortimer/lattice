use std::path::Path;

use crate::error::Result;
use crate::registry::NamespaceRegistry;
use crate::types::ResourceStat;

/// Inspect authority and materialization for a workspace-relative path.
pub fn resource_stat(workspace_root: &Path, path: &str) -> Result<ResourceStat> {
    let registry = NamespaceRegistry::open(workspace_root)?;
    registry.resource_stat(path)
}

/// Like [`resource_stat`], but registers local files that are not yet in the registry.
pub fn resource_stat_or_register(workspace_root: &Path, path: &str) -> Result<ResourceStat> {
    let mut registry = NamespaceRegistry::open(workspace_root)?;
    match registry.resource_stat(path) {
        Ok(stat) => Ok(stat),
        Err(crate::error::Error::ResourceNotFound { .. }) => {
            registry.ensure_local_file(path)?;
            registry.save()?;
            registry.resource_stat(path)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AuthorityMode;
    use tempfile::tempdir;

    #[test]
    fn resource_stat_returns_local_authority_for_default_file() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        registry.ensure_local_file("hello.txt").unwrap();
        registry.save().unwrap();

        let stat = resource_stat(dir.path(), "hello.txt").unwrap();
        assert_eq!(stat.path, "hello.txt");
        assert_eq!(stat.authority, AuthorityMode::Local);
    }

    #[test]
    fn resource_stat_or_register_inserts_missing_local_file() {
        let dir = tempdir().unwrap();
        let stat = resource_stat_or_register(dir.path(), "notes/a.md").unwrap();
        assert_eq!(stat.path, "notes/a.md");
        assert_eq!(stat.authority, AuthorityMode::Local);
        assert!(NamespaceRegistry::open(dir.path())
            .unwrap()
            .resource_stat("notes/a.md")
            .is_ok());
    }
}
