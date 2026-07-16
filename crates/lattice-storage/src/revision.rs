use std::io::Read;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

/// Identity of one materialized state of a resource.
///
/// The [`hash`](ResourceRevision::hash) is content-addressed and stable:
/// identical bytes always produce the same hash regardless of when or where
/// they were written, which is what lets optimistic revision checks work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRevision {
    /// `"sha256:<hex>"` of the content bytes.
    pub hash: String,
    pub len: u64,
    pub modified: SystemTime,
}

impl ResourceRevision {
    /// Compute the revision for `data`, tagging it with the given
    /// modification time. The hash covers only the bytes, not the time.
    pub(crate) fn compute(data: &[u8], modified: SystemTime) -> Self {
        let digest = Sha256::digest(data);
        ResourceRevision {
            hash: format!("sha256:{}", hex::encode(digest)),
            len: data.len() as u64,
            modified,
        }
    }
}

/// Stream a content hash without loading the resource into memory.
pub fn sha256_reader(mut reader: impl Read) -> std::io::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}
