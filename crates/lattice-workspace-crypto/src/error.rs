//! Explicit error surface for workspace crypto.

use thiserror::Error;

/// Errors from DEK lifecycle, keystore, or AEAD helpers.
#[derive(Debug, Error)]
pub enum Error {
    #[error("workspace is locked; unlock before encrypt/decrypt")]
    Locked,

    #[error("workspace is already unlocked")]
    AlreadyUnlocked,

    #[error("no wrapped DEK found for workspace `{0}`")]
    MissingDek(String),

    #[error("wrapped DEK has invalid length (expected {expected}, got {got})")]
    InvalidDekLength { expected: usize, got: usize },

    #[error("keystore error: {0}")]
    Keystore(String),

    #[error("encryption failed: {0}")]
    Encrypt(String),

    #[error("decryption failed: {0}")]
    Decrypt(String),

    #[error("ciphertext too short")]
    CiphertextTooShort,

    #[error("backup payload error: {message}")]
    BackupPayload { message: String },

    #[error("backup envelope error: {message}")]
    BackupEnvelope { message: String },
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
