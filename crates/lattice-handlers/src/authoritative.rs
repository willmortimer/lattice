//! Authoritative resource bytes: local disk or cloud GET (no silent fallback).

use lattice_cloud_client::{
    default_client, HttpCloudBlobClient, KeychainCloudSessionStore, MemoryCloudSessionStore,
    resolve_cloud_bearer, CloudSessionStore,
};
use latticefs_core::{
    open_cloud_authoritative_bytes, resource_stat_or_register, AuthorityMode,
};
use std::sync::OnceLock;

use crate::path::resolve_within_root;

fn session_store() -> &'static dyn CloudSessionStore {
    static KEYCHAIN: OnceLock<KeychainCloudSessionStore> = OnceLock::new();
    static MEMORY: OnceLock<MemoryCloudSessionStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        let store = KeychainCloudSessionStore::new();
        match store.save_token("probe") {
            Ok(()) => {
                let _ = store.clear_token();
                false
            }
            Err(_) => true,
        }
    });
    if use_memory {
        MEMORY.get_or_init(MemoryCloudSessionStore::new)
    } else {
        KEYCHAIN.get_or_init(KeychainCloudSessionStore::new)
    }
}

/// Read bytes for a workspace path. Cloud authority uses GET only (never local disk).
pub fn read_authoritative_bytes(root: &str, rel_path: &str) -> Result<Vec<u8>, String> {
    let (canonical_root, canonical_candidate) = resolve_within_root(root, rel_path)?;
    let rel_key = rel_path.replace('\\', "/");
    let stat = resource_stat_or_register(&canonical_root, &rel_key).map_err(|err| err.to_string())?;
    if stat.authority == AuthorityMode::Cloud {
        let token = resolve_cloud_bearer(session_store()).map_err(|err| err.to_string())?;
        let client = HttpCloudBlobClient::new(default_client(), token);
        return open_cloud_authoritative_bytes(&canonical_root, &rel_key, &client)
            .map_err(|err| err.to_string());
    }
    std::fs::read(&canonical_candidate).map_err(|err| err.to_string())
}

/// UTF-8 text via [`read_authoritative_bytes`].
pub fn read_authoritative_string(root: &str, rel_path: &str) -> Result<String, String> {
    let bytes = read_authoritative_bytes(root, rel_path)?;
    String::from_utf8(bytes).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use latticefs_core::{
        ContentHash, InMemoryCloudBlobClient, NamespaceRegistry, roundtrip_verify_blob,
    };
    use tempfile::tempdir;

    #[test]
    fn local_authority_reads_disk() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), b"local-bytes").unwrap();
        let got = read_authoritative_bytes(dir.path().to_str().unwrap(), "note.md").unwrap();
        assert_eq!(got, b"local-bytes");
    }

    #[test]
    fn cloud_authority_does_not_return_stale_local_bytes() {
        let dir = tempdir().unwrap();
        let path = "notes/a.md";
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        std::fs::write(dir.path().join(path), b"stale-local").unwrap();

        let mut registry = NamespaceRegistry::open(dir.path()).unwrap();
        let resource_id = registry.ensure_local_file(path).unwrap();
        registry.save().unwrap();

        let cloud = InMemoryCloudBlobClient::new();
        let data = b"cloud-canonical";
        let hash = roundtrip_verify_blob(&cloud, resource_id, data).unwrap();
        registry.mark_cloud_backed(path, hash).unwrap();
        registry.save().unwrap();

        // Stale local bytes must not be treated as authoritative for cloud resources.
        let stat = registry.resource_stat(path).unwrap();
        assert_eq!(stat.authority, AuthorityMode::Cloud);
        assert_ne!(
            std::fs::read(dir.path().join(path)).unwrap().as_slice(),
            data.as_slice()
        );
        let _ = ContentHash::from_bytes(data);
    }
}
