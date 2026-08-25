//! Backup envelope that wraps the workspace DEK outside LWBK ciphertext.
//!
//! Cloud stores the envelope as an opaque blob. Format:
//! `LWBE` magic, version `1`, u16 LE wrapped-DEK length, wrapped DEK bytes,
//! then the existing DEK-encrypted LWBK body.

use crate::aead::{decrypt_blob, encrypt_blob};
use crate::dek::Dek;
use crate::error::{Error, Result};

/// Envelope magic (`LWBE`).
pub const ENVELOPE_MAGIC: &[u8; 4] = b"LWBE";
/// Envelope version byte.
pub const ENVELOPE_VERSION: u8 = 1;

const HEADER_LEN: usize = 4 + 1 + 2;

/// Parsed `LWBE` envelope: wrapped DEK plus inner DEK-encrypted LWBK blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEnvelope {
    pub wrapped_dek: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Whether `bytes` starts with the `LWBE` magic.
pub fn is_backup_envelope(bytes: &[u8]) -> bool {
    bytes.starts_with(ENVELOPE_MAGIC)
}

/// Wrap `dek` under the account wrap key using the same AEAD as content.
pub fn wrap_dek(wrap_key: &Dek, dek: &Dek) -> Result<Vec<u8>> {
    encrypt_blob(wrap_key, dek.as_bytes())
}

/// Unwrap a DEK previously produced by [`wrap_dek`].
pub fn unwrap_dek(wrap_key: &Dek, wrapped: &[u8]) -> Result<Dek> {
    let plain = decrypt_blob(wrap_key, wrapped)?;
    Dek::try_from_slice(&plain)
}

/// Seal wrapped-DEK bytes and inner ciphertext into an `LWBE` envelope.
pub fn seal_backup_envelope(wrapped_dek: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.is_empty() {
        return Err(Error::BackupEnvelope {
            message: "ciphertext body is empty".into(),
        });
    }
    let wrapped_len: u16 = u16::try_from(wrapped_dek.len()).map_err(|_| Error::BackupEnvelope {
        message: "wrapped DEK exceeds u16 length".into(),
    })?;
    let mut out = Vec::with_capacity(HEADER_LEN + wrapped_dek.len() + ciphertext.len());
    out.extend_from_slice(ENVELOPE_MAGIC);
    out.push(ENVELOPE_VERSION);
    out.extend_from_slice(&wrapped_len.to_le_bytes());
    out.extend_from_slice(wrapped_dek);
    out.extend_from_slice(ciphertext);
    Ok(out)
}

/// Parse an `LWBE` envelope without unwrapping the DEK.
pub fn parse_backup_envelope(bytes: &[u8]) -> Result<BackupEnvelope> {
    if bytes.len() < HEADER_LEN {
        return Err(Error::BackupEnvelope {
            message: "truncated backup envelope".into(),
        });
    }
    if &bytes[..4] != ENVELOPE_MAGIC {
        return Err(Error::BackupEnvelope {
            message: "invalid backup envelope magic (expected LWBE)".into(),
        });
    }
    let version = bytes[4];
    if version != ENVELOPE_VERSION {
        return Err(Error::BackupEnvelope {
            message: format!("unsupported backup envelope version {version}"),
        });
    }
    let wrapped_len = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
    let wrapped_start = HEADER_LEN;
    let wrapped_end =
        wrapped_start
            .checked_add(wrapped_len)
            .ok_or_else(|| Error::BackupEnvelope {
                message: "wrapped DEK length overflow".into(),
            })?;
    if bytes.len() < wrapped_end {
        return Err(Error::BackupEnvelope {
            message: "truncated wrapped DEK".into(),
        });
    }
    let ciphertext = bytes[wrapped_end..].to_vec();
    if ciphertext.is_empty() {
        return Err(Error::BackupEnvelope {
            message: "ciphertext body is empty".into(),
        });
    }
    Ok(BackupEnvelope {
        wrapped_dek: bytes[wrapped_start..wrapped_end].to_vec(),
        ciphertext,
    })
}

/// Unwrap the DEK and return it with the inner ciphertext.
///
/// On unwrap failure the caller must not provision a replacement DEK.
pub fn open_backup_envelope(wrap_key: &Dek, bytes: &[u8]) -> Result<(Dek, Vec<u8>)> {
    let envelope = parse_backup_envelope(bytes)?;
    let dek = unwrap_dek(wrap_key, &envelope.wrapped_dek)?;
    Ok((dek, envelope.ciphertext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dek::generate_dek;
    use crate::keystore::MemoryKeystore;
    use crate::session::WorkspaceCryptoSession;

    #[test]
    fn envelope_round_trip_with_empty_keystore() {
        let wrap_key = generate_dek();
        let source_store = MemoryKeystore::new();
        let mut source = WorkspaceCryptoSession::new(source_store);
        source.provision("ws-restore").unwrap();
        let plaintext = b"LWBK-stand-in";
        let ciphertext = source.encrypt_blob(plaintext).unwrap();
        let wrapped = source.wrap_unlocked_dek(&wrap_key).unwrap();
        let envelope = seal_backup_envelope(&wrapped, &ciphertext).unwrap();
        assert!(is_backup_envelope(&envelope));
        assert!(envelope.starts_with(ENVELOPE_MAGIC));

        let dest_store = MemoryKeystore::new();
        let mut dest = WorkspaceCryptoSession::new(dest_store);
        assert!(matches!(
            dest.unlock("ws-restore"),
            Err(Error::MissingDek(_))
        ));

        let (imported, inner) = open_backup_envelope(&wrap_key, &envelope).unwrap();
        dest.import_dek("ws-restore", imported).unwrap();
        let recovered = dest.decrypt_blob(&inner).unwrap();
        assert_eq!(recovered, plaintext);
        dest.lock();
        dest.unlock("ws-restore").unwrap();
        assert_eq!(dest.decrypt_blob(&inner).unwrap(), plaintext);
    }

    #[test]
    fn legacy_ciphertext_still_decrypts_without_envelope() {
        let store = MemoryKeystore::new();
        let mut session = WorkspaceCryptoSession::new(store);
        session.provision("ws-legacy").unwrap();
        let plaintext = b"legacy-lwbk";
        let ciphertext = session.encrypt_blob(plaintext).unwrap();
        assert!(!is_backup_envelope(&ciphertext));
        session.lock();
        session.unlock("ws-legacy").unwrap();
        assert_eq!(session.decrypt_blob(&ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn unwrap_with_wrong_wrap_key_fails() {
        let wrap_key = generate_dek();
        let other_key = generate_dek();
        let dek = generate_dek();
        let ciphertext = encrypt_blob(&dek, b"secret").unwrap();
        let envelope =
            seal_backup_envelope(&wrap_dek(&wrap_key, &dek).unwrap(), &ciphertext).unwrap();
        assert!(open_backup_envelope(&other_key, &envelope).is_err());
    }
}
