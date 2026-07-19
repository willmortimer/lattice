//! Map bridge error codes to `lattice_voice::SpeechError`.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use lattice_voice::SpeechError;

use crate::ffi::{
    LATTICE_VOICE_ERR_ALREADY_PREPARED, LATTICE_VOICE_ERR_CANCELLED,
    LATTICE_VOICE_ERR_INTERNAL, LATTICE_VOICE_ERR_INVALID_ARG, LATTICE_VOICE_ERR_NOT_FOUND,
    LATTICE_VOICE_ERR_NOT_PREPARED, LATTICE_VOICE_ERR_SESSION, LATTICE_VOICE_ERR_UNSUPPORTED,
    LATTICE_VOICE_OK,
};

/// Result of calling into the native bridge.
pub(crate) type BridgeResult<T> = Result<T, SpeechError>;

/// Fail closed when the linked bridge reports an unexpected ABI version.
pub fn ensure_abi_version(expected: u32, actual: u32) -> Result<(), SpeechError> {
    if actual == expected {
        Ok(())
    } else {
        Err(SpeechError::provider(format!(
            "LatticeVoiceBridge ABI mismatch: expected {expected}, got {actual}"
        )))
    }
}

pub(crate) fn map_status(code: i32, context: &str) -> BridgeResult<()> {
    if code == LATTICE_VOICE_OK {
        Ok(())
    } else {
        Err(map_code(code, context))
    }
}

pub(crate) fn map_code(code: i32, context: &str) -> SpeechError {
    let detail = match code {
        LATTICE_VOICE_ERR_INVALID_ARG => "invalid argument",
        LATTICE_VOICE_ERR_NOT_PREPARED => "engine is not prepared",
        LATTICE_VOICE_ERR_ALREADY_PREPARED => "engine is already prepared",
        LATTICE_VOICE_ERR_SESSION => "session error",
        LATTICE_VOICE_ERR_CANCELLED => "session was cancelled",
        LATTICE_VOICE_ERR_INTERNAL => "internal bridge error",
        LATTICE_VOICE_ERR_UNSUPPORTED => "unsupported operation",
        LATTICE_VOICE_ERR_NOT_FOUND => "resource not found",
        _ => "unknown bridge error",
    };

    SpeechError::provider(format!("{context}: {detail} (code {code})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_mismatch_is_fail_closed() {
        let err = ensure_abi_version(1, 2).unwrap_err();
        assert!(matches!(err, SpeechError::Provider { .. }));
        assert!(err.to_string().contains("ABI mismatch"));
    }

    #[test]
    fn abi_version_match_passes() {
        ensure_abi_version(1, 1).unwrap();
    }

    #[test]
    fn maps_known_bridge_codes() {
        let err = map_code(LATTICE_VOICE_ERR_CANCELLED, "cancel");
        assert!(err.to_string().contains("cancelled"));
    }
}
