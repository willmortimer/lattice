//! GitHub App connector handlers (Tauri-free).
//!
//! Auth and connect live in the CLI (`lattice github`). The desktop only lists
//! existing bindings and opens read-only extract files.

use std::path::Path;
use std::sync::{OnceLock};

use lattice_connectors::{
    disconnect_repo, list_bindings, list_checkout_tree, read_checkout_file, refresh_repo,
    CheckoutEntry, CheckoutFile, ConnectedRepoSummary, KeychainTokenStore, MemoryTokenStore,
    TokenMaterial, TokenStore,
};

fn token_store() -> &'static dyn TokenStore {
    // Prefer keychain; fall back to process-local memory when keychain is
    // unavailable (CI / sandboxed environments).
    static KEYCHAIN: OnceLock<KeychainTokenStore> = OnceLock::new();
    static MEMORY: OnceLock<MemoryTokenStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        let store = KeychainTokenStore::new();
        let probe_key = "lattice.github.probe";
        match store.set(
            probe_key,
            &TokenMaterial {
                access_token: "probe".into(),
                refresh_token: None,
                expires_in: None,
                token_type: None,
            },
        ) {
            Ok(()) => {
                let _ = store.delete(probe_key);
                false
            }
            Err(_) => true,
        }
    });
    if use_memory {
        MEMORY.get_or_init(MemoryTokenStore::new)
    } else {
        KEYCHAIN.get_or_init(KeychainTokenStore::new)
    }
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

pub fn github_list_bindings(root: String) -> Result<Vec<ConnectedRepoSummary>, String> {
    list_bindings(Path::new(&root)).map_err(map_err)
}

pub fn github_refresh_repo(
    root: String,
    binding_id: String,
) -> Result<ConnectedRepoSummary, String> {
    refresh_repo(Path::new(&root), token_store(), &binding_id).map_err(map_err)
}

pub fn github_disconnect_repo(root: String, binding_id: String) -> Result<(), String> {
    disconnect_repo(Path::new(&root), token_store(), &binding_id).map_err(map_err)
}

pub fn github_list_checkout_tree(
    root: String,
    binding_id: String,
) -> Result<Vec<CheckoutEntry>, String> {
    list_checkout_tree(Path::new(&root), &binding_id).map_err(map_err)
}

pub fn github_read_checkout_file(
    root: String,
    binding_id: String,
    rel_path: String,
) -> Result<CheckoutFile, String> {
    read_checkout_file(Path::new(&root), &binding_id, &rel_path).map_err(map_err)
}
