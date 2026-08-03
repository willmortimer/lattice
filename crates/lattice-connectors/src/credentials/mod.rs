//! Token storage: OS keychain in production, memory for tests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[cfg(target_os = "macos")]
mod secitem_macos;
#[cfg(target_os = "macos")]
pub use secitem_macos::{
    AppGroupSecItemTokenStore, MigratingAppGroupTokenStore, LATTICE_APP_GROUP,
    LATTICE_KEYCHAIN_ACCESS_GROUP,
};

pub const GITHUB_TOKEN_SERVICE: &str = "lattice.github";
/// Keychain account for the CLI/desktop user access token from GitHub login.
pub const GITHUB_USER_TOKEN_KEY: &str = "lattice.github.user";
/// Ephemeral probe account; must not overlap user or binding keys.
pub const GITHUB_PROBE_KEY: &str = "lattice.github.probe";

pub const GITLAB_TOKEN_SERVICE: &str = "lattice.gitlab";
/// Keychain account for the CLI/desktop user access token from GitLab login.
pub const GITLAB_USER_TOKEN_KEY: &str = "lattice.gitlab.user";
/// Ephemeral probe account; must not overlap user or binding keys.
pub const GITLAB_PROBE_KEY: &str = "lattice.gitlab.probe";

/// Keychain service name for a connector provider id (`github`, `gitlab`, …).
pub fn token_service_for(provider: &str) -> String {
    format!("lattice.{provider}")
}

/// User-session keychain account for a connector provider.
pub fn user_token_key_for(provider: &str) -> String {
    format!("lattice.{provider}.user")
}

/// Per-binding keychain account.
pub fn binding_token_key_for(provider: &str, binding_id: &str) -> String {
    format!("lattice.{provider}.{binding_id}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenMaterial {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
}

pub trait TokenStore: Send + Sync {
    fn set(&self, key: &str, material: &TokenMaterial) -> Result<()>;
    fn get(&self, key: &str) -> Result<Option<TokenMaterial>>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Returns `true` when the OS credential store accepts a write/delete cycle for `probe_key`.
///
/// Handlers use this once per process to decide between [`production_token_store`] and
/// [`MemoryTokenStore`]. The probe key must be dedicated (never a user-session account).
pub fn probe_token_store_writable(service: &str, probe_key: &str) -> bool {
    let store = production_token_store(service);
    let material = TokenMaterial {
        access_token: "probe".into(),
        refresh_token: None,
        expires_in: None,
        token_type: None,
    };
    match store.set(probe_key, &material) {
        Ok(()) => {
            let _ = store.delete(probe_key);
            true
        }
        Err(_) => false,
    }
}

/// Preferred production store: App Group SecItem on macOS (with legacy migrate),
/// plain keyring elsewhere / when entitlements are missing.
pub fn production_token_store(service: impl Into<String>) -> Box<dyn TokenStore> {
    let service = service.into();
    #[cfg(target_os = "macos")]
    {
        Box::new(MigratingAppGroupTokenStore::with_service(service))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(KeychainTokenStore::with_service(service))
    }
}

/// In-memory store for unit tests and environments without a keychain.
#[derive(Debug, Default, Clone)]
pub struct MemoryTokenStore {
    inner: Arc<Mutex<HashMap<String, TokenMaterial>>>,
}

impl MemoryTokenStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenStore for MemoryTokenStore {
    fn set(&self, key: &str, material: &TokenMaterial) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::credentials("token store lock poisoned"))?
            .insert(key.to_string(), material.clone());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<TokenMaterial>> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| Error::credentials("token store lock poisoned"))?
            .get(key)
            .cloned())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::credentials("token store lock poisoned"))?
            .remove(key);
        Ok(())
    }
}

/// OS keychain-backed store. Values are JSON-serialized [`TokenMaterial`].
pub struct KeychainTokenStore {
    service: String,
}

impl KeychainTokenStore {
    pub fn new() -> Self {
        Self {
            service: GITHUB_TOKEN_SERVICE.to_string(),
        }
    }

    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl Default for KeychainTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStore for KeychainTokenStore {
    fn set(&self, key: &str, material: &TokenMaterial) -> Result<()> {
        let entry = keyring::Entry::new(&self.service, key)
            .map_err(|err| Error::credentials(err.to_string()))?;
        let payload = serde_json::to_string(material)?;
        entry
            .set_password(&payload)
            .map_err(|err| Error::credentials(err.to_string()))
    }

    fn get(&self, key: &str) -> Result<Option<TokenMaterial>> {
        let entry = keyring::Entry::new(&self.service, key)
            .map_err(|err| Error::credentials(err.to_string()))?;
        match entry.get_password() {
            Ok(payload) => {
                let material: TokenMaterial = serde_json::from_str(&payload)?;
                Ok(Some(material))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(Error::credentials(err.to_string())),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(&self.service, key)
            .map_err(|err| Error::credentials(err.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::credentials(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SERVICE: &str = "lattice.test.credentials";
    const TEST_KEY: &str = "round-trip";

    fn sample_material() -> TokenMaterial {
        TokenMaterial {
            access_token: "tok".into(),
            refresh_token: Some("ref".into()),
            expires_in: Some(3600),
            token_type: Some("bearer".into()),
        }
    }

    #[test]
    fn memory_round_trip() {
        let store = MemoryTokenStore::new();
        let material = sample_material();
        store.set("k1", &material).unwrap();
        assert_eq!(store.get("k1").unwrap().unwrap().access_token, "tok");
        store.delete("k1").unwrap();
        assert!(store.get("k1").unwrap().is_none());
    }

    #[test]
    fn keychain_round_trip_when_writable() {
        if !probe_token_store_writable(TEST_SERVICE, "lattice.test.probe") {
            eprintln!("skipping keychain_round_trip_when_writable: OS keyring not writable");
            return;
        }
        let store = KeychainTokenStore::with_service(TEST_SERVICE);
        let material = sample_material();
        store.set(TEST_KEY, &material).unwrap();
        let loaded = store.get(TEST_KEY).unwrap().expect("stored token");
        assert_eq!(loaded, material);
        store.delete(TEST_KEY).unwrap();
        assert!(store.get(TEST_KEY).unwrap().is_none());
    }

    #[test]
    fn production_token_store_round_trip_when_writable() {
        if !probe_token_store_writable(TEST_SERVICE, "lattice.test.probe") {
            eprintln!("skipping production_token_store_round_trip_when_writable: OS keyring not writable");
            return;
        }
        let store = production_token_store(TEST_SERVICE);
        let material = sample_material();
        store.set(TEST_KEY, &material).unwrap();
        let loaded = store.get(TEST_KEY).unwrap().expect("stored token");
        assert_eq!(loaded, material);
        store.delete(TEST_KEY).unwrap();
        assert!(store.get(TEST_KEY).unwrap().is_none());
    }

    #[test]
    fn probe_uses_dedicated_key_not_user_accounts() {
        assert_ne!(GITHUB_PROBE_KEY, GITHUB_USER_TOKEN_KEY);
        assert_ne!(GITLAB_PROBE_KEY, GITLAB_USER_TOKEN_KEY);
    }
}
