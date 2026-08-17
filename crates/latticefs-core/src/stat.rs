use std::path::Path;

use crate::cloud::{fetch_cloud_blob, roundtrip_verify_blob, CloudBlobClient};
use crate::error::{Error, Result};
use crate::registry::NamespaceRegistry;
use crate::types::{AuthorityMode, HydrationInputDigest, ResourceAuthority, ResourceStat};

/// Inspect authority and materialization for a workspace-relative path.
pub fn resource_stat(workspace_root: &Path, path: &str) -> Result<ResourceStat> {
    let registry = NamespaceRegistry::open(workspace_root)?;
    registry.resource_stat(path)
}

/// Set durable editing authority for a workspace-relative path and return the updated stat.
///
/// Registers the path with [`NamespaceRegistry::ensure_local_file`] when missing, mirroring
/// [`resource_stat_or_register`].
pub fn set_resource_authority(
    workspace_root: &Path,
    path: &str,
    authority: ResourceAuthority,
) -> Result<ResourceStat> {
    let mut registry = NamespaceRegistry::open(workspace_root)?;
    match registry.resource_stat(path) {
        Ok(_) => {}
        Err(crate::error::Error::ResourceNotFound { .. }) => {
            registry.ensure_local_file(path)?;
        }
        Err(err) => return Err(err),
    }
    registry.set_resource_authority(path, authority)?;
    registry.save()?;
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

/// On proposal accept, mint a [`crate::ResourceVersionId`] per path and store hydration digests.
///
/// No-op when `paths` is empty. `hydration_inputs` may be empty for ordinary accepts.
/// Paths are normalized registry keys.
pub fn attach_accept_hydration_lineage(
    workspace_root: &Path,
    paths: &[String],
    hydration_inputs: &[HydrationInputDigest],
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut registry = NamespaceRegistry::open(workspace_root)?;
    let digests = hydration_inputs.to_vec();
    for path in paths {
        registry.record_accepted_version(path, digests.clone())?;
    }
    registry.save()?;
    Ok(())
}

/// PUT → GET verify against cloud, then record [`AuthorityMode::Cloud`] in the registry.
pub fn materialize_to_cloud(
    workspace_root: &Path,
    path: &str,
    data: &[u8],
    client: &dyn CloudBlobClient,
) -> Result<ResourceStat> {
    let mut registry = NamespaceRegistry::open(workspace_root)?;
    let resource_id = match registry.resource_stat(path) {
        Ok(stat) => stat.resource_id,
        Err(Error::ResourceNotFound { .. }) => {
            let id = registry.ensure_local_file(path)?;
            registry.save()?;
            id
        }
        Err(err) => return Err(err),
    };
    let hash = roundtrip_verify_blob(client, resource_id, data)?;
    registry.mark_cloud_backed(path, hash)?;
    registry.save()?;
    registry.resource_stat(path)
}

/// Fetch canonical bytes for a cloud-backed resource.
///
/// Returns an error when the registry authority is not cloud or when the cloud
/// GET fails. Does not read local file bytes as a fallback.
pub fn open_cloud_authoritative_bytes(
    workspace_root: &Path,
    path: &str,
    client: &dyn CloudBlobClient,
) -> Result<Vec<u8>> {
    let registry = NamespaceRegistry::open(workspace_root)?;
    let stat = registry.resource_stat(path)?;
    if stat.authority != AuthorityMode::Cloud {
        return Err(Error::NotCloudAuthoritative {
            path: stat.path,
        });
    }
    fetch_cloud_blob(
        client,
        stat.resource_id,
        stat.content_hash.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::InMemoryCloudBlobClient;
    use crate::registry::NamespaceRegistry;
    use crate::types::{AuthorityMode, ContentHash, ResourceAuthority, ResourceId};
    use tempfile::tempdir;

    struct FailingGetBlobClient;

    impl crate::cloud::CloudBlobClient for FailingGetBlobClient {
        fn put_blob(
            &self,
            _resource_id: crate::types::ResourceId,
            data: &[u8],
        ) -> crate::Result<ContentHash> {
            ContentHash::from_bytes(data)
        }

        fn get_blob(
            &self,
            _resource_id: crate::types::ResourceId,
        ) -> crate::Result<Vec<u8>> {
            Err(crate::error::Error::CloudBlob {
                message: "offline".into(),
            })
        }
    }

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

    #[test]
    fn materialize_to_cloud_sets_authority() {
        let dir = tempdir().unwrap();
        let client = InMemoryCloudBlobClient::new();
        let data = b"cloud-canonical-bytes";
        let stat = materialize_to_cloud(dir.path(), "notes/a.md", data, &client).unwrap();
        assert_eq!(stat.authority, AuthorityMode::Cloud);
        assert_eq!(stat.content_hash, Some(ContentHash::from_bytes(data).unwrap()));
        let reopened = resource_stat(dir.path(), "notes/a.md").unwrap();
        assert_eq!(reopened.authority, AuthorityMode::Cloud);
    }

    #[test]
    fn materialize_to_cloud_leaves_local_authority_when_get_fails() {
        let dir = tempdir().unwrap();
        let client = FailingGetBlobClient;
        let data = b"cloud-canonical-bytes";
        let err = materialize_to_cloud(dir.path(), "notes/a.md", data, &client).unwrap_err();
        assert!(matches!(err, Error::CloudBlob { .. }));

        let stat = resource_stat(dir.path(), "notes/a.md").unwrap();
        assert_eq!(stat.authority, AuthorityMode::Local);
    }

    #[test]
    fn open_cloud_authoritative_bytes_errors_on_failed_get_not_local_disk() {
        let dir = tempdir().unwrap();
        let good = InMemoryCloudBlobClient::new();
        let data = b"cloud-canonical-bytes";
        materialize_to_cloud(dir.path(), "notes/a.md", data, &good).unwrap();

        let file_path = dir.path().join("notes/a.md");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, b"stale-local-bytes").unwrap();

        let failing = FailingGetBlobClient;
        let err = open_cloud_authoritative_bytes(dir.path(), "notes/a.md", &failing).unwrap_err();
        assert!(matches!(err, Error::CloudBlob { .. }));
    }

    #[test]
    fn open_cloud_authoritative_bytes_rejects_local_authority() {
        let dir = tempdir().unwrap();
        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        registry.ensure_local_file("notes/a.md").unwrap();
        registry.save().unwrap();

        let client = InMemoryCloudBlobClient::new();
        let err =
            open_cloud_authoritative_bytes(dir.path(), "notes/a.md", &client).unwrap_err();
        assert!(matches!(err, Error::NotCloudAuthoritative { .. }));
    }

    #[test]
    fn set_resource_authority_collaborative_round_trips() {
        let dir = tempdir().unwrap();
        let doc_id = ResourceId::new();
        let stat = set_resource_authority(
            dir.path(),
            "notes/a.md",
            ResourceAuthority::Collaborative {
                doc_id,
                materialized_revision: None,
            },
        )
        .unwrap();
        assert_eq!(
            stat.resource_authority,
            ResourceAuthority::Collaborative {
                doc_id,
                materialized_revision: None,
            }
        );

        let reopened = NamespaceRegistry::open(dir.path()).unwrap();
        let reread = reopened.resource_stat("notes/a.md").unwrap();
        assert_eq!(reread.resource_authority, stat.resource_authority);
    }

    #[test]
    fn attach_accept_hydration_lineage_mints_version_without_hydration_inputs() {
        let dir = tempdir().unwrap();
        attach_accept_hydration_lineage(dir.path(), &["Reports/out.txt".into()], &[]).unwrap();
        let stat = resource_stat(dir.path(), "Reports/out.txt").unwrap();
        assert!(stat.version_id.is_some());
        assert!(stat.hydration_inputs.is_empty());
    }

    #[test]
    fn attach_accept_hydration_lineage_writes_inspectable_digests() {
        let dir = tempdir().unwrap();
        let digests = vec![crate::HydrationInputDigest {
            path: "input/hello.txt".into(),
            content_hash: "abc123".into(),
            resource_id: None,
        }];
        attach_accept_hydration_lineage(
            dir.path(),
            &["Reports/out.txt".into()],
            &digests,
        )
        .unwrap();
        let stat = resource_stat(dir.path(), "Reports/out.txt").unwrap();
        assert!(stat.version_id.is_some());
        assert_eq!(stat.hydration_inputs, digests);
    }
}
