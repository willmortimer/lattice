use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{normalize_path_key, Error, Result};
use crate::types::{
    AuthorityMode, ContentHash, MaterializationState, NamespaceEntry, ResourceId, ResourceStat,
    ResourceVersionId,
};

pub const OPERATIONAL_DIR: &str = ".lattice";
pub const REGISTRY_FILENAME: &str = "resource-registry.json";

/// Persisted metadata for one registered resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ResourceRecord {
    resource_id: ResourceId,
    authority: AuthorityMode,
    materialization: MaterializationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_hash: Option<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version_id: Option<ResourceVersionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegistryDocument {
    version: u32,
    entries: BTreeMap<String, ResourceRecord>,
}

impl Default for RegistryDocument {
    fn default() -> Self {
        Self {
            version: 1,
            entries: BTreeMap::new(),
        }
    }
}

/// Workspace path → stable [`ResourceId`] registry persisted under `.lattice/`.
#[derive(Debug, Clone)]
pub struct NamespaceRegistry {
    workspace_root: PathBuf,
    document: RegistryDocument,
}

impl NamespaceRegistry {
    pub fn open(workspace_root: impl Into<PathBuf>) -> Result<Self> {
        let workspace_root = workspace_root.into();
        let registry_path = Self::registry_path(&workspace_root);
        let document = if registry_path.exists() {
            let raw = fs::read_to_string(&registry_path)?;
            serde_json::from_str(&raw)?
        } else {
            RegistryDocument::default()
        };
        Ok(Self {
            workspace_root,
            document,
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn registry_path(workspace_root: &Path) -> PathBuf {
        workspace_root
            .join(OPERATIONAL_DIR)
            .join(REGISTRY_FILENAME)
    }

    pub fn save(&self) -> Result<()> {
        let registry_path = Self::registry_path(&self.workspace_root);
        if let Some(parent) = registry_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(&self.document)?;
        fs::write(registry_path, raw)?;
        Ok(())
    }

    pub fn ensure_local_file(&mut self, path: &str) -> Result<ResourceId> {
        let key = normalize_path_key(path)?;
        if let Some(record) = self.document.entries.get(&key) {
            return Ok(record.resource_id);
        }
        let record = ResourceRecord {
            resource_id: ResourceId::new(),
            authority: AuthorityMode::Local,
            materialization: MaterializationState::Pinned,
            content_hash: None,
            version_id: None,
        };
        let resource_id = record.resource_id;
        self.document.entries.insert(key, record);
        Ok(resource_id)
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<ResourceId> {
        let from_key = normalize_path_key(from)?;
        let to_key = normalize_path_key(to)?;
        if from_key == to_key {
            return self
                .document
                .entries
                .get(&from_key)
                .map(|record| record.resource_id)
                .ok_or_else(|| Error::RenameSourceNotFound { from: from_key });
        }
        let record = self
            .document
            .entries
            .remove(&from_key)
            .ok_or_else(|| Error::RenameSourceNotFound {
                from: from_key.clone(),
            })?;
        if self.document.entries.contains_key(&to_key) {
            self.document.entries.insert(from_key, record);
            return Err(Error::RenameDestinationExists { to: to_key });
        }
        let resource_id = record.resource_id;
        self.document.entries.insert(to_key, record);
        Ok(resource_id)
    }

    pub fn resource_stat(&self, path: &str) -> Result<ResourceStat> {
        let key = normalize_path_key(path)?;
        let record = self
            .document
            .entries
            .get(&key)
            .ok_or_else(|| Error::ResourceNotFound { path: key.clone() })?;
        Ok(ResourceStat {
            resource_id: record.resource_id,
            path: key,
            authority: record.authority,
            materialization: record.materialization,
            content_hash: record.content_hash.clone(),
            version_id: record.version_id,
        })
    }

    pub fn namespace_entry(&self, path: &str) -> Result<NamespaceEntry> {
        let stat = self.resource_stat(path)?;
        Ok(NamespaceEntry {
            path: stat.path,
            resource_id: stat.resource_id,
        })
    }

    pub fn set_content_hash(&mut self, path: &str, content_hash: ContentHash) -> Result<()> {
        let key = normalize_path_key(path)?;
        let record = self
            .document
            .entries
            .get_mut(&key)
            .ok_or_else(|| Error::ResourceNotFound { path: key.clone() })?;
        record.content_hash = Some(content_hash);
        Ok(())
    }

    pub fn update_content_hash_from_bytes(&mut self, path: &str, data: &[u8]) -> Result<ContentHash> {
        let hash = ContentHash::from_bytes(data)?;
        self.set_content_hash(path, hash.clone())?;
        Ok(hash)
    }

    /// Record that canonical bytes for `path` live in cloud after a verified upload.
    pub fn mark_cloud_backed(&mut self, path: &str, content_hash: ContentHash) -> Result<()> {
        let key = normalize_path_key(path)?;
        let record = self
            .document
            .entries
            .get_mut(&key)
            .ok_or_else(|| Error::ResourceNotFound { path: key.clone() })?;
        record.authority = AuthorityMode::Cloud;
        record.materialization = MaterializationState::MetadataOnly;
        record.content_hash = Some(content_hash);
        Ok(())
    }

    pub fn entries(&self) -> impl Iterator<Item = NamespaceEntry> + '_ {
        self.document.entries.iter().map(|(path, record)| NamespaceEntry {
            path: path.clone(),
            resource_id: record.resource_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rename_preserves_resource_id() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        let id = registry.ensure_local_file("notes/a.md").unwrap();
        let renamed = registry.rename("notes/a.md", "notes/b.md").unwrap();
        assert_eq!(id, renamed);
        assert!(registry.resource_stat("notes/a.md").is_err());
        let stat = registry.resource_stat("notes/b.md").unwrap();
        assert_eq!(stat.resource_id, id);
    }

    #[test]
    fn registry_persists_across_reopen() {
        let dir = tempdir().unwrap();
        let id = {
            let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
            let id = registry.ensure_local_file("doc.md").unwrap();
            registry.save().unwrap();
            id
        };
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat("doc.md").unwrap();
        assert_eq!(stat.resource_id, id);
        assert_eq!(stat.authority, AuthorityMode::Local);
        assert_eq!(stat.materialization, MaterializationState::Pinned);
    }

    #[test]
    fn rename_destination_conflict_restores_source() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        registry.ensure_local_file("a.md").unwrap();
        registry.ensure_local_file("b.md").unwrap();
        let err = registry.rename("a.md", "b.md").unwrap_err();
        assert!(matches!(err, Error::RenameDestinationExists { .. }));
        assert!(registry.resource_stat("a.md").is_ok());
    }
}
