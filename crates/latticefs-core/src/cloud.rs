use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};
use crate::types::{ContentHash, ResourceId};

/// Authenticated cloud blob I/O for one account-scoped resource.
pub trait CloudBlobClient: Send + Sync {
    fn put_blob(&self, resource_id: ResourceId, data: &[u8]) -> Result<ContentHash>;
    fn get_blob(&self, resource_id: ResourceId) -> Result<Vec<u8>>;
}

/// In-memory test double with no HTTP.
#[derive(Debug, Default, Clone)]
pub struct InMemoryCloudBlobClient {
    blobs: Arc<Mutex<HashMap<ResourceId, Vec<u8>>>>,
}

impl InMemoryCloudBlobClient {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CloudBlobClient for InMemoryCloudBlobClient {
    fn put_blob(&self, resource_id: ResourceId, data: &[u8]) -> Result<ContentHash> {
        let hash = ContentHash::from_bytes(data)?;
        let mut blobs = self.blobs.lock().unwrap();
        if blobs.contains_key(&resource_id) {
            return Err(Error::BlobAlreadyExists {
                resource_id: resource_id.to_string(),
            });
        }
        blobs.insert(resource_id, data.to_vec());
        Ok(hash)
    }

    fn get_blob(&self, resource_id: ResourceId) -> Result<Vec<u8>> {
        self.blobs
            .lock()
            .unwrap()
            .get(&resource_id)
            .cloned()
            .ok_or_else(|| Error::BlobNotFound {
                resource_id: resource_id.to_string(),
            })
    }
}

/// GET canonical bytes from cloud and optionally verify against `expected_hash`.
///
/// Never reads local workspace files; callers must not substitute disk bytes on error.
pub fn fetch_cloud_blob(
    client: &dyn CloudBlobClient,
    resource_id: ResourceId,
    expected_hash: Option<&ContentHash>,
) -> Result<Vec<u8>> {
    let fetched = client.get_blob(resource_id)?;
    if let Some(expected) = expected_hash {
        let fetched_hash = ContentHash::from_bytes(&fetched)?;
        if fetched_hash != *expected {
            return Err(Error::BlobHashMismatch {
                expected: expected.clone(),
                actual: fetched_hash,
            });
        }
    }
    Ok(fetched)
}

/// PUT → GET and verify the fetched bytes match the stored hash.
pub fn roundtrip_verify_blob(
    client: &dyn CloudBlobClient,
    resource_id: ResourceId,
    data: &[u8],
) -> Result<ContentHash> {
    let hash = client.put_blob(resource_id, data)?;
    fetch_cloud_blob(client, resource_id, Some(&hash))?;
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_roundtrip_verifies_hash() {
        let client = InMemoryCloudBlobClient::new();
        let id = ResourceId::new();
        let data = b"opaque-cloud-bytes";
        let hash = roundtrip_verify_blob(&client, id, data).unwrap();
        assert_eq!(hash, ContentHash::from_bytes(data).unwrap());
    }

    #[test]
    fn in_memory_rejects_duplicate_put() {
        let client = InMemoryCloudBlobClient::new();
        let id = ResourceId::new();
        client.put_blob(id, b"a").unwrap();
        let err = client.put_blob(id, b"b").unwrap_err();
        assert!(matches!(err, Error::BlobAlreadyExists { .. }));
    }

    struct FailingGetBlobClient;

    impl CloudBlobClient for FailingGetBlobClient {
        fn put_blob(&self, _resource_id: ResourceId, data: &[u8]) -> Result<ContentHash> {
            ContentHash::from_bytes(data).map_err(|_| Error::InvalidContentHash {
                value: "bad".into(),
            })
        }

        fn get_blob(&self, _resource_id: ResourceId) -> Result<Vec<u8>> {
            Err(Error::CloudBlob {
                message: "network unreachable".into(),
            })
        }
    }

    #[test]
    fn fetch_cloud_blob_surfaces_get_failure() {
        let client = FailingGetBlobClient;
        let id = ResourceId::new();
        let err = fetch_cloud_blob(&client, id, None).unwrap_err();
        assert!(matches!(err, Error::CloudBlob { .. }));
        assert!(err.to_string().contains("network unreachable"));
    }

    struct PutOkGetFailClient;

    impl CloudBlobClient for PutOkGetFailClient {
        fn put_blob(&self, _resource_id: ResourceId, data: &[u8]) -> Result<ContentHash> {
            ContentHash::from_bytes(data)
        }

        fn get_blob(&self, _resource_id: ResourceId) -> Result<Vec<u8>> {
            Err(Error::CloudBlob {
                message: "cloud API error (401): invalid session".into(),
            })
        }
    }

    #[test]
    fn roundtrip_verify_blob_fails_when_get_fails() {
        let client = PutOkGetFailClient;
        let id = ResourceId::new();
        let err = roundtrip_verify_blob(&client, id, b"payload").unwrap_err();
        assert!(matches!(err, Error::CloudBlob { .. }));
    }
}
