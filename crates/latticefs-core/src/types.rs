use std::fmt;
use std::io::Cursor;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Stable identity for a workspace resource. Independent of path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(Uuid);

impl ResourceId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ResourceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Identity of one accepted version of a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceVersionId(Uuid);

impl ResourceVersionId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ResourceVersionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ResourceVersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Content-addressed digest in the `sha256:<hex>` form used across Lattice.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.starts_with("sha256:") && value.len() > "sha256:".len() {
            Ok(Self(value))
        } else {
            Err(Error::InvalidContentHash { value })
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let hash = lattice_storage::sha256_reader(Cursor::new(data)).map_err(Error::Io)?;
        Self::new(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Who owns the canonical bytes for a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityMode {
    Local,
    Cloud,
    External,
    ImmutableImport,
}

/// Whether bytes are present locally and how they are retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializationState {
    MetadataOnly,
    Cached,
    Pinned,
    Evicted,
}

/// One path binding in the workspace namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceEntry {
    pub path: String,
    pub resource_id: ResourceId,
}

/// Authority and materialization snapshot for one resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceStat {
    pub resource_id: ResourceId,
    pub path: String,
    pub authority: AuthorityMode,
    pub materialization: MaterializationState,
    pub content_hash: Option<ContentHash>,
    pub version_id: Option<ResourceVersionId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_accepts_sha256_prefix() {
        let hash = ContentHash::new("sha256:abc").unwrap();
        assert_eq!(hash.as_str(), "sha256:abc");
    }

    #[test]
    fn content_hash_rejects_invalid_prefix() {
        assert!(ContentHash::new("md5:abc").is_err());
    }
}
