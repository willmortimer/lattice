//! Keystore trait and backends for wrapped DEK storage.
//!
//! The OS keychain (or an in-memory mock) is the wrapping layer: the DEK never
//! lives on disk in plaintext. Biometrics / LocalAuthentication gate access to
//! the store; they are not encryption themselves (ADR 0038 / ADR 0049).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};

/// Persist and retrieve wrapped DEK bytes keyed by workspace id.
pub trait Keystore: Send + Sync {
    /// Store wrapped DEK bytes for `workspace_id`, replacing any prior value.
    fn store_wrapped_dek(&self, workspace_id: &str, wrapped: &[u8]) -> Result<()>;

    /// Load wrapped DEK bytes, or `None` if absent.
    fn load_wrapped_dek(&self, workspace_id: &str) -> Result<Option<Vec<u8>>>;

    /// Delete wrapped DEK bytes. Missing keys are not an error.
    fn delete_wrapped_dek(&self, workspace_id: &str) -> Result<()>;
}

/// In-memory keystore for unit tests and environments without a keychain.
#[derive(Debug, Default, Clone)]
pub struct MemoryKeystore {
    inner: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MemoryKeystore {
    /// Empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Keystore for MemoryKeystore {
    fn store_wrapped_dek(&self, workspace_id: &str, wrapped: &[u8]) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::Keystore("memory keystore lock poisoned".into()))?
            .insert(workspace_id.to_string(), wrapped.to_vec());
        Ok(())
    }

    fn load_wrapped_dek(&self, workspace_id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| Error::Keystore("memory keystore lock poisoned".into()))?
            .get(workspace_id)
            .cloned())
    }

    fn delete_wrapped_dek(&self, workspace_id: &str) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::Keystore("memory keystore lock poisoned".into()))?
            .remove(workspace_id);
        Ok(())
    }
}

/// OS keychain service name for workspace DEKs (ADR 0038).
pub const WORKSPACE_DEK_SERVICE: &str = "lattice.workspace.dek";

/// Keychain account / item name for a workspace id.
pub fn dek_account_for(workspace_id: &str) -> String {
    format!("lattice.workspace.dek.{workspace_id}")
}

/// Thin OS keychain adapter via `keyring` platform backends.
///
/// Enabled with the `keychain` feature. DEK bytes are stored as opaque secret
/// material; the OS credential store is the wrap.
#[cfg(feature = "keychain")]
pub struct KeychainKeystore {
    service: String,
}

#[cfg(feature = "keychain")]
impl KeychainKeystore {
    /// Default Lattice workspace-DEK service.
    pub fn new() -> Self {
        Self {
            service: WORKSPACE_DEK_SERVICE.to_string(),
        }
    }

    /// Custom keychain service name (tests / multi-tenant hosts).
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

#[cfg(feature = "keychain")]
impl Default for KeychainKeystore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "keychain")]
impl Keystore for KeychainKeystore {
    fn store_wrapped_dek(&self, workspace_id: &str, wrapped: &[u8]) -> Result<()> {
        let account = dek_account_for(workspace_id);
        let entry = keyring::Entry::new(&self.service, &account)
            .map_err(|err| Error::Keystore(err.to_string()))?;
        // keyring password APIs are UTF-8; store as base64-free hex for binary DEKs.
        let encoded = hex_encode(wrapped);
        entry
            .set_password(&encoded)
            .map_err(|err| Error::Keystore(err.to_string()))
    }

    fn load_wrapped_dek(&self, workspace_id: &str) -> Result<Option<Vec<u8>>> {
        let account = dek_account_for(workspace_id);
        let entry = keyring::Entry::new(&self.service, &account)
            .map_err(|err| Error::Keystore(err.to_string()))?;
        match entry.get_password() {
            Ok(encoded) => {
                let bytes = hex_decode(&encoded)?;
                Ok(Some(bytes))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(Error::Keystore(err.to_string())),
        }
    }

    fn delete_wrapped_dek(&self, workspace_id: &str) -> Result<()> {
        let account = dek_account_for(workspace_id);
        let entry = keyring::Entry::new(&self.service, &account)
            .map_err(|err| Error::Keystore(err.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::Keystore(err.to_string())),
        }
    }
}

#[cfg(feature = "keychain")]
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(feature = "keychain")]
fn hex_decode(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(Error::Keystore("invalid hex length".into()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

#[cfg(feature = "keychain")]
fn hex_nibble(c: u8) -> Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(Error::Keystore("invalid hex digit".into())),
    }
}
