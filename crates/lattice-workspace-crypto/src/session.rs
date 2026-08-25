//! Lock / unlock session that keeps the DEK in Rust memory only.
//!
//! App lock / presence (ADR 0049) is session privacy, not encryption. This
//! module must not be wired into `presence.rs`, `app_lock.rs`, or capture/**.

use crate::aead::{decrypt_blob, encrypt_blob};
use crate::dek::{generate_dek, Dek};
use crate::error::{Error, Result};
use crate::keystore::Keystore;

/// In-process workspace crypto session. The webview never holds the DEK.
pub struct WorkspaceCryptoSession<K: Keystore> {
    keystore: K,
    workspace_id: Option<String>,
    dek: Option<Dek>,
}

impl<K: Keystore> WorkspaceCryptoSession<K> {
    /// Create a locked session bound to `keystore`.
    pub fn new(keystore: K) -> Self {
        Self {
            keystore,
            workspace_id: None,
            dek: None,
        }
    }

    /// Whether a DEK is currently held in memory.
    pub fn is_unlocked(&self) -> bool {
        self.dek.is_some()
    }

    /// Workspace id of the unlocked DEK, if any.
    pub fn unlocked_workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }

    /// Generate a DEK, wrap it in the keystore, and leave the session unlocked.
    pub fn provision(&mut self, workspace_id: &str) -> Result<()> {
        if self.dek.is_some() {
            return Err(Error::AlreadyUnlocked);
        }
        let dek = generate_dek();
        self.keystore
            .store_wrapped_dek(workspace_id, dek.as_bytes())?;
        self.workspace_id = Some(workspace_id.to_string());
        self.dek = Some(dek);
        Ok(())
    }

    /// Persist an existing DEK (restore import) and unlock the session with it.
    ///
    /// Replaces any previously stored DEK for `workspace_id`. Does not generate
    /// a new key — restore must fail rather than provision on unwrap errors.
    pub fn import_dek(&mut self, workspace_id: &str, dek: Dek) -> Result<()> {
        self.keystore
            .store_wrapped_dek(workspace_id, dek.as_bytes())?;
        self.workspace_id = Some(workspace_id.to_string());
        self.dek = Some(dek);
        Ok(())
    }

    /// Wrap the unlocked DEK under `wrap_key` for a backup envelope.
    pub fn wrap_unlocked_dek(&self, wrap_key: &Dek) -> Result<Vec<u8>> {
        let dek = self.dek.as_ref().ok_or(Error::Locked)?;
        encrypt_blob(wrap_key, dek.as_bytes())
    }

    /// Load the wrapped DEK from the keystore into memory.
    pub fn unlock(&mut self, workspace_id: &str) -> Result<()> {
        if self.dek.is_some() {
            return Err(Error::AlreadyUnlocked);
        }
        let wrapped = self
            .keystore
            .load_wrapped_dek(workspace_id)?
            .ok_or_else(|| Error::MissingDek(workspace_id.to_string()))?;
        let dek = Dek::try_from_slice(&wrapped)?;
        self.workspace_id = Some(workspace_id.to_string());
        self.dek = Some(dek);
        Ok(())
    }

    /// Drop the in-memory DEK. Wrapped material remains in the keystore.
    pub fn lock(&mut self) {
        self.dek = None;
        self.workspace_id = None;
    }

    /// Remove wrapped DEK from the keystore and clear memory.
    pub fn destroy(&mut self, workspace_id: &str) -> Result<()> {
        self.keystore.delete_wrapped_dek(workspace_id)?;
        if self.workspace_id.as_deref() == Some(workspace_id) {
            self.lock();
        }
        Ok(())
    }

    /// Encrypt a blob with the unlocked DEK.
    pub fn encrypt_blob(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let dek = self.dek.as_ref().ok_or(Error::Locked)?;
        encrypt_blob(dek, plaintext)
    }

    /// Decrypt a blob with the unlocked DEK.
    pub fn decrypt_blob(&self, blob: &[u8]) -> Result<Vec<u8>> {
        let dek = self.dek.as_ref().ok_or(Error::Locked)?;
        decrypt_blob(dek, blob)
    }
}
