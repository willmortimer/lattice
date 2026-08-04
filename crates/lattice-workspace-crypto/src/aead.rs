//! Authenticated encryption helpers under a DEK (AES-256-GCM).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;

use crate::dek::Dek;
use crate::error::{Error, Result};

/// AES-GCM nonce length in bytes.
pub const NONCE_LEN: usize = 12;

/// Encrypt `plaintext` under `dek`. Output is `nonce || ciphertext||tag`.
pub fn encrypt_blob(dek: &Dek, plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(dek.as_bytes())
        .map_err(|err| Error::Encrypt(err.to_string()))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|err| Error::Encrypt(err.to_string()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `blob` (`nonce || ciphertext||tag`) under `dek`.
pub fn decrypt_blob(dek: &Dek, blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() <= NONCE_LEN {
        return Err(Error::CiphertextTooShort);
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(dek.as_bytes())
        .map_err(|err| Error::Decrypt(err.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|err| Error::Decrypt(err.to_string()))
}
