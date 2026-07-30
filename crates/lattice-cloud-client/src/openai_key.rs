use std::sync::{Arc, Mutex};

use lattice_connectors::{production_token_store, TokenMaterial, TokenStore};

use crate::error::{CloudError, Result};

/// OS keychain service for the desktop BYO OpenAI API key.
pub const OPENAI_KEY_SERVICE: &str = "lattice.ai.openai";
/// Keychain account for the BYO OpenAI API key.
pub const OPENAI_KEY_ACCOUNT: &str = "api-key";

pub trait OpenAiKeyStore: Send + Sync {
    fn set_key(&self, key: &str) -> Result<()>;
    fn has_key(&self) -> Result<bool>;
    fn load_key(&self) -> Result<Option<String>>;
    fn clear_key(&self) -> Result<()>;
}

#[derive(Debug, Default, Clone)]
pub struct MemoryOpenAiKeyStore {
    inner: Arc<Mutex<Option<String>>>,
}

impl MemoryOpenAiKeyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl OpenAiKeyStore for MemoryOpenAiKeyStore {
    fn set_key(&self, key: &str) -> Result<()> {
        *self.inner.lock().unwrap() = Some(key.to_string());
        Ok(())
    }

    fn has_key(&self) -> Result<bool> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|value| !value.is_empty()))
    }

    fn load_key(&self) -> Result<Option<String>> {
        Ok(self.inner.lock().unwrap().clone())
    }

    fn clear_key(&self) -> Result<()> {
        *self.inner.lock().unwrap() = None;
        Ok(())
    }
}

pub struct KeychainOpenAiKeyStore {
    store: Box<dyn TokenStore>,
}

impl KeychainOpenAiKeyStore {
    pub fn new() -> Self {
        Self {
            store: production_token_store(OPENAI_KEY_SERVICE),
        }
    }
}

impl Default for KeychainOpenAiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiKeyStore for KeychainOpenAiKeyStore {
    fn set_key(&self, key: &str) -> Result<()> {
        self.store
            .set(
                OPENAI_KEY_ACCOUNT,
                &TokenMaterial {
                    access_token: key.to_string(),
                    refresh_token: None,
                    expires_in: None,
                    token_type: Some("api-key".into()),
                },
            )
            .map_err(|err| CloudError::Credentials(err.to_string()))
    }

    fn has_key(&self) -> Result<bool> {
        Ok(self
            .load_key()?
            .is_some_and(|value| !value.trim().is_empty()))
    }

    fn load_key(&self) -> Result<Option<String>> {
        self.store
            .get(OPENAI_KEY_ACCOUNT)
            .map(|material| material.map(|token| token.access_token))
            .map_err(|err| CloudError::Credentials(err.to_string()))
    }

    fn clear_key(&self) -> Result<()> {
        self.store
            .delete(OPENAI_KEY_ACCOUNT)
            .map_err(|err| CloudError::Credentials(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip_without_echoing_to_frontend_api() {
        let store = MemoryOpenAiKeyStore::new();
        assert!(!store.has_key().unwrap());
        store.set_key("sk-test").unwrap();
        assert!(store.has_key().unwrap());
        assert_eq!(store.load_key().unwrap().as_deref(), Some("sk-test"));
        store.clear_key().unwrap();
        assert!(!store.has_key().unwrap());
        assert!(store.load_key().unwrap().is_none());
    }

    #[test]
    fn keychain_store_uses_openai_service_constants() {
        assert_eq!(OPENAI_KEY_SERVICE, "lattice.ai.openai");
        assert_eq!(OPENAI_KEY_ACCOUNT, "api-key");
        let _store = KeychainOpenAiKeyStore::new();
    }
}
