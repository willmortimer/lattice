use std::path::Path;

use crate::error::Result;
use crate::registry::NamespaceRegistry;
use crate::types::ResourceStat;

/// Inspect authority and materialization for a workspace-relative path.
pub fn resource_stat(workspace_root: &Path, path: &str) -> Result<ResourceStat> {
    let registry = NamespaceRegistry::open(workspace_root)?;
    registry.resource_stat(path)
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
}
