//! Parse and validate collab document ids as LatticeFS ResourceIds.

use std::str::FromStr;

use latticefs_core::ResourceId;

use crate::error::{Error, Result};

/// Parse a collab `doc_id` as a registry [`ResourceId`] UUID.
///
/// Rejects synthetic keys such as `path:Notes.md` — collaboration is keyed by
/// stable registry identity only (ADR 0055).
pub fn parse_doc_resource_id(raw: &str) -> Result<ResourceId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidDocId {
            raw: raw.to_string(),
        });
    }
    if trimmed.contains(':') {
        // Catch `path:…` and other scheme-prefixed synthetic ids before UUID parse.
        return Err(Error::InvalidDocId {
            raw: raw.to_string(),
        });
    }
    ResourceId::from_str(trimmed).map_err(|_| Error::InvalidDocId {
        raw: raw.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_uuid_resource_id() {
        let id = ResourceId::new();
        let parsed = parse_doc_resource_id(&id.to_string()).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn rejects_path_scheme() {
        let err = parse_doc_resource_id("path:Notes.md").unwrap_err();
        assert!(matches!(err, Error::InvalidDocId { .. }));
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            parse_doc_resource_id("  ").unwrap_err(),
            Error::InvalidDocId { .. }
        ));
    }

    #[test]
    fn rejects_non_uuid() {
        assert!(matches!(
            parse_doc_resource_id("not-a-uuid").unwrap_err(),
            Error::InvalidDocId { .. }
        ));
    }
}
