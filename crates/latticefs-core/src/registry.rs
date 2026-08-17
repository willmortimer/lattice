use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{normalize_path_key, Error, Result};
use crate::types::{
    AuthorityMode, ContentHash, HydrationInputDigest, MaterializationState, NamespaceEntry,
    ResourceAuthority, ResourceId, ResourceStat, ResourceVersionId,
};

pub const OPERATIONAL_DIR: &str = ".lattice";
pub const REGISTRY_FILENAME: &str = "resource-registry.json";

/// Persisted metadata for one registered resource (portable LatticeFS control fields).
///
/// Device materialization is derived at read time and is not written back (ADR 0068).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ResourceRecord {
    resource_id: ResourceId,
    authority: AuthorityMode,
    #[serde(default)]
    resource_authority: ResourceAuthority,
    /// Accepted from older registries; never serialized going forward.
    #[serde(default, skip_serializing)]
    materialization: MaterializationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_hash: Option<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version_id: Option<ResourceVersionId>,
    /// Provenance stub: KernelFS hydration digests from the accepted proposal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hydration_inputs: Vec<HydrationInputDigest>,
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
            resource_authority: ResourceAuthority::PlainFile,
            materialization: MaterializationState::Pinned,
            content_hash: None,
            version_id: None,
            hydration_inputs: Vec::new(),
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
            materialization: derive_materialization(record.authority),
            resource_authority: record.resource_authority.clone(),
            content_hash: record.content_hash.clone(),
            version_id: record.version_id,
            hydration_inputs: record.hydration_inputs.clone(),
        })
    }

    pub fn namespace_entry(&self, path: &str) -> Result<NamespaceEntry> {
        let stat = self.resource_stat(path)?;
        Ok(NamespaceEntry {
            path: stat.path,
            resource_id: stat.resource_id,
        })
    }

    pub fn set_resource_authority(
        &mut self,
        path: &str,
        resource_authority: ResourceAuthority,
    ) -> Result<()> {
        let key = normalize_path_key(path)?;
        let record = self
            .document
            .entries
            .get_mut(&key)
            .ok_or_else(|| Error::ResourceNotFound { path: key.clone() })?;
        record.resource_authority = resource_authority;
        Ok(())
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

    /// Mint a [`ResourceVersionId`] and attach hydration digests as accept lineage.
    ///
    /// Registers the path when missing. Replaces any prior version stub for this path.
    pub fn record_accepted_version(
        &mut self,
        path: &str,
        hydration_inputs: Vec<HydrationInputDigest>,
    ) -> Result<ResourceVersionId> {
        let key = normalize_path_key(path)?;
        if !self.document.entries.contains_key(&key) {
            self.ensure_local_file(&key)?;
        }
        let version_id = ResourceVersionId::new();
        let record = self
            .document
            .entries
            .get_mut(&key)
            .ok_or_else(|| Error::ResourceNotFound { path: key.clone() })?;
        record.version_id = Some(version_id);
        record.hydration_inputs = hydration_inputs;
        Ok(version_id)
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

    /// Resolve a stable [`ResourceId`] back to its registered path key.
    pub fn path_for_resource_id(&self, resource_id: ResourceId) -> Option<String> {
        self.document
            .entries
            .iter()
            .find(|(_, record)| record.resource_id == resource_id)
            .map(|(path, _)| path.clone())
    }

    /// Drop a path registration, returning the prior id when present.
    pub fn remove(&mut self, path: &str) -> Result<Option<ResourceId>> {
        let key = normalize_path_key(path)?;
        Ok(self
            .document
            .entries
            .remove(&key)
            .map(|record| record.resource_id))
    }
}

fn derive_materialization(authority: AuthorityMode) -> MaterializationState {
    match authority {
        AuthorityMode::Local | AuthorityMode::ImmutableImport => MaterializationState::Pinned,
        AuthorityMode::Cloud | AuthorityMode::External => MaterializationState::MetadataOnly,
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
    fn path_for_resource_id_and_remove_round_trip() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        let id = registry.ensure_local_file("notes/a.md").unwrap();
        assert_eq!(
            registry.path_for_resource_id(id).as_deref(),
            Some("notes/a.md")
        );
        assert_eq!(registry.remove("notes/a.md").unwrap(), Some(id));
        assert!(registry.path_for_resource_id(id).is_none());
        assert_eq!(registry.remove("notes/a.md").unwrap(), None);
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

    #[test]
    fn record_accepted_version_sets_version_and_hydration_inputs() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        let digests = vec![HydrationInputDigest {
            path: "hello.txt".into(),
            content_hash: "0f328ae687eb8fd2acfa3a910bb6722eff43f8a7dbd08e53e572ae37a0c5d7a5"
                .into(),
            resource_id: Some("res-1".into()),
        }];
        let version_id = registry
            .record_accepted_version("Reports/out.txt", digests.clone())
            .unwrap();
        registry.save().unwrap();

        let reopened = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = reopened.resource_stat("Reports/out.txt").unwrap();
        assert_eq!(stat.version_id, Some(version_id));
        assert_eq!(stat.hydration_inputs, digests);
        let raw = fs::read_to_string(NamespaceRegistry::registry_path(dir.path())).unwrap();
        assert!(raw.contains("hydration_inputs") || raw.contains("contentHash"));
        assert!(raw.contains("0f328ae687eb8fd2acfa3a910bb6722eff43f8a7dbd08e53e572ae37a0c5d7a5"));
    }

    #[test]
    fn portable_registry_omits_materialization_field() {
        let dir = tempdir().unwrap();
        {
            let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
            registry.ensure_local_file("doc.md").unwrap();
            registry.save().unwrap();
        }
        let raw = fs::read_to_string(NamespaceRegistry::registry_path(dir.path())).unwrap();
        assert!(!raw.contains("materialization"), "portable registry must not serialize materialization: {raw}");
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        assert_eq!(registry.resource_stat("doc.md").unwrap().materialization, MaterializationState::Pinned);
    }

    #[test]
    fn legacy_materialization_field_is_ignored_on_load() {
        let dir = tempdir().unwrap();
        let registry_path = NamespaceRegistry::registry_path(dir.path());
        fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        let resource_id = ResourceId::new();
        fs::write(&registry_path, format!(r#"{{"version":1,"entries":{{"legacy.md":{{"resource_id":"{resource_id}","authority":"cloud","materialization":"cached"}}}}}}"#)).unwrap();
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat("legacy.md").unwrap();
        assert_eq!(stat.resource_id, resource_id);
        assert_eq!(stat.authority, AuthorityMode::Cloud);
        assert_eq!(stat.materialization, MaterializationState::MetadataOnly);
        registry.save().unwrap();
        assert!(!fs::read_to_string(registry_path).unwrap().contains("materialization"));
    }

    #[test]
    fn legacy_registry_without_resource_authority_defaults_to_plain_file() {
        let dir = tempdir().unwrap();
        let registry_path = NamespaceRegistry::registry_path(dir.path());
        fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        let resource_id = ResourceId::new();
        fs::write(
            &registry_path,
            format!(
                r#"{{"version":1,"entries":{{"legacy.md":{{"resource_id":"{resource_id}","authority":"local"}}}}}}"#
            ),
        )
        .unwrap();
        let registry = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = registry.resource_stat("legacy.md").unwrap();
        assert_eq!(stat.resource_authority, ResourceAuthority::PlainFile);
    }

    #[test]
    fn set_resource_authority_collaborative_round_trips() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        registry.ensure_local_file("notes/a.md").unwrap();
        let doc_id = ResourceId::new();
        let revision = ResourceVersionId::new();
        registry
            .set_resource_authority(
                "notes/a.md",
                ResourceAuthority::Collaborative {
                    doc_id,
                    materialized_revision: Some(revision),
                },
            )
            .unwrap();
        registry.save().unwrap();

        let reopened = NamespaceRegistry::open(dir.path()).unwrap();
        let stat = reopened.resource_stat("notes/a.md").unwrap();
        assert_eq!(
            stat.resource_authority,
            ResourceAuthority::Collaborative {
                doc_id,
                materialized_revision: Some(revision),
            }
        );
        let raw = fs::read_to_string(NamespaceRegistry::registry_path(dir.path())).unwrap();
        assert!(raw.contains("collaborative"));
        assert!(raw.contains(&doc_id.to_string()));
    }
}
