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

/// PUT → GET and verify the fetched bytes match the stored hash.
pub fn roundtrip_verify_blob(
    client: &dyn CloudBlobClient,
    resource_id: ResourceId,
    data: &[u8],
) -> Result<ContentHash> {
    let hash = client.put_blob(resource_id, data)?;
    let fetched = client.get_blob(resource_id)?;
    let fetched_hash = ContentHash::from_bytes(&fetched)?;
    if fetched_hash != hash {
        return Err(Error::BlobHashMismatch {
            expected: hash,
            actual: fetched_hash,
        });
    }
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
}
