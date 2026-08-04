//! Data-encryption key (DEK) generation and ownership.

use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// AES-256 key length in bytes.
pub const DEK_LEN: usize = 32;

/// Workspace data-encryption key. Zeroized on drop; never leave Rust trust boundary.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Dek {
    bytes: [u8; DEK_LEN],
}

impl Dek {
    /// Generate a fresh random DEK.
    pub fn generate() -> Self {
        let mut bytes = [0u8; DEK_LEN];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Build a DEK from raw bytes (e.g. after keystore unwrap).
    pub fn from_bytes(bytes: [u8; DEK_LEN]) -> Self {
        Self { bytes }
    }

    /// Try to build a DEK from a byte slice.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, crate::Error> {
        if bytes.len() != DEK_LEN {
            return Err(crate::Error::InvalidDekLength {
                expected: DEK_LEN,
                got: bytes.len(),
            });
        }
        let mut arr = [0u8; DEK_LEN];
        arr.copy_from_slice(bytes);
        Ok(Self::from_bytes(arr))
    }

    /// Borrow the raw key material for AEAD. Callers must not persist or export.
    pub fn as_bytes(&self) -> &[u8; DEK_LEN] {
        &self.bytes
    }
}

impl std::fmt::Debug for Dek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Dek([REDACTED])")
    }
}

/// Generate a fresh workspace DEK.
pub fn generate_dek() -> Dek {
    Dek::generate()
}
