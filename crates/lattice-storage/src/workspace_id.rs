//! Stable workspace identity shared across catalogs and LatticeFS.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Enduring workspace identity from `lattice.yaml` (`id` field).
///
/// Profile, scheduler, and LatticeFS catalogs must share this type rather than
/// inventing parallel path-keyed identities (ADR 0068).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid.to_string())
    }

    pub fn new() -> Self {
        Self::from_uuid(Uuid::now_v7())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for WorkspaceId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for WorkspaceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(s.trim())?;
        Ok(Self::from_uuid(uuid))
    }
}

impl From<Uuid> for WorkspaceId {
    fn from(uuid: Uuid) -> Self {
        Self::from_uuid(uuid)
    }
}

impl From<WorkspaceId> for String {
    fn from(value: WorkspaceId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_uuid_string() {
        let id = WorkspaceId::new();
        let parsed: WorkspaceId = id.as_str().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn rejects_non_uuid() {
        assert!("not-a-uuid".parse::<WorkspaceId>().is_err());
    }
}
