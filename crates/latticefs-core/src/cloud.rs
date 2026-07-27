use crate::error::Result;
use crate::types::{ContentHash, ResourceId};

/// Test double for future cloud blob I/O against `lattice-server`.
pub trait MockCloudBlobClient: Send + Sync {
    fn put_blob(&self, resource_id: ResourceId, data: &[u8]) -> Result<ContentHash>;
    fn get_blob(&self, resource_id: ResourceId) -> Result<Vec<u8>>;
}
