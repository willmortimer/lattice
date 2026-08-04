//! Workspace encryption DEK lifecycle and AEAD helpers (ADR 0038).
//!
//! # Boundary
//!
//! - Generate a DEK in Rust, wrap/store it via [`Keystore`] (OS keychain or mock).
//! - Lock / unlock keeps the DEK in process memory only; the webview never holds it.
//! - Encrypt / decrypt content only on Rust storage / command paths.
//!
//! App lock and presence (ADR 0049) are **not** encryption. Do not couple this
//! crate to `presence.rs`, `app_lock.rs`, or capture/**.

mod aead;
mod dek;
mod error;
mod keystore;
mod session;

pub use aead::{decrypt_blob, encrypt_blob, NONCE_LEN};
pub use dek::{generate_dek, Dek, DEK_LEN};
pub use error::{Error, Result};
pub use keystore::{dek_account_for, Keystore, MemoryKeystore, WORKSPACE_DEK_SERVICE};
#[cfg(feature = "keychain")]
pub use keystore::KeychainKeystore;
pub use session::WorkspaceCryptoSession;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_dek_is_correct_length() {
        let dek = generate_dek();
        assert_eq!(dek.as_bytes().len(), DEK_LEN);
    }

    #[test]
    fn lock_unlock_round_trip() {
        let store = MemoryKeystore::new();
        let mut session = WorkspaceCryptoSession::new(store);

        session.provision("ws-1").expect("provision");
        assert!(session.is_unlocked());
        assert_eq!(session.unlocked_workspace_id(), Some("ws-1"));

        let ciphertext = session
            .encrypt_blob(b"hello workspace")
            .expect("encrypt while unlocked");

        session.lock();
        assert!(!session.is_unlocked());
        assert!(matches!(
            session.encrypt_blob(b"nope"),
            Err(Error::Locked)
        ));

        session.unlock("ws-1").expect("unlock");
        assert!(session.is_unlocked());
        let plain = session.decrypt_blob(&ciphertext).expect("decrypt");
        assert_eq!(plain, b"hello workspace");
    }

    #[test]
    fn encrypt_decrypt_with_dek() {
        let dek = generate_dek();
        let blob = encrypt_blob(&dek, b"secret payload").expect("encrypt");
        assert!(blob.len() > NONCE_LEN);
        let plain = decrypt_blob(&dek, &blob).expect("decrypt");
        assert_eq!(plain, b"secret payload");
    }

    #[test]
    fn decrypt_rejects_short_ciphertext() {
        let dek = generate_dek();
        assert!(matches!(
            decrypt_blob(&dek, &[0u8; 4]),
            Err(Error::CiphertextTooShort)
        ));
    }

    #[test]
    fn unlock_missing_dek_errors() {
        let store = MemoryKeystore::new();
        let mut session = WorkspaceCryptoSession::new(store);
        assert!(matches!(
            session.unlock("missing"),
            Err(Error::MissingDek(_))
        ));
    }

    #[test]
    fn destroy_removes_wrapped_dek() {
        let store = MemoryKeystore::new();
        let mut session = WorkspaceCryptoSession::new(store);
        session.provision("ws-x").unwrap();
        session.destroy("ws-x").unwrap();
        assert!(!session.is_unlocked());
        assert!(matches!(
            session.unlock("ws-x"),
            Err(Error::MissingDek(_))
        ));
    }

    #[test]
    fn memory_keystore_round_trip() {
        let store = MemoryKeystore::new();
        store.store_wrapped_dek("a", &[1, 2, 3]).unwrap();
        assert_eq!(store.load_wrapped_dek("a").unwrap(), Some(vec![1, 2, 3]));
        store.delete_wrapped_dek("a").unwrap();
        assert_eq!(store.load_wrapped_dek("a").unwrap(), None);
    }
}
