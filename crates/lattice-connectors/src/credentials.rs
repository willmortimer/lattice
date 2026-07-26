//! Token storage: OS keychain in production, memory for tests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const GITHUB_TOKEN_SERVICE: &str = "lattice.github";
/// Keychain account for the CLI/desktop user access token from GitHub login.
pub const GITHUB_USER_TOKEN_KEY: &str = "lattice.github.user";

pub const GITLAB_TOKEN_SERVICE: &str = "lattice.gitlab";
/// Keychain account for the CLI/desktop user access token from GitLab login.
pub const GITLAB_USER_TOKEN_KEY: &str = "lattice.gitlab.user";

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

    #[test]
    fn memory_round_trip() {
        let store = MemoryTokenStore::new();
        let material = TokenMaterial {
            access_token: "tok".into(),
            refresh_token: Some("ref".into()),
            expires_in: Some(3600),
            token_type: Some("bearer".into()),
        };
        store.set("k1", &material).unwrap();
        assert_eq!(store.get("k1").unwrap().unwrap().access_token, "tok");
        store.delete("k1").unwrap();
        assert!(store.get("k1").unwrap().is_none());
    }
}
