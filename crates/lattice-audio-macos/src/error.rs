//! Map bridge error codes to `lattice_audio::CaptureError`.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use lattice_audio::CaptureError;

use crate::ffi::{
    LATTICE_AUDIO_ERR_ALREADY_RUNNING, LATTICE_AUDIO_ERR_DEVICE, LATTICE_AUDIO_ERR_INTERNAL,
    LATTICE_AUDIO_ERR_INVALID_ARG, LATTICE_AUDIO_ERR_NOT_ARMED, LATTICE_AUDIO_ERR_NOT_RUNNING,
    LATTICE_AUDIO_ERR_PERMISSION, LATTICE_AUDIO_ERR_UNSUPPORTED, LATTICE_AUDIO_OK,
};

pub(crate) type BridgeResult<T> = Result<T, CaptureError>;

/// Fail closed when the linked bridge reports an unexpected ABI version.
pub fn ensure_abi_version(expected: u32, actual: u32) -> Result<(), CaptureError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CaptureError::provider(format!(
            "LatticeAudioBridge ABI mismatch: expected {expected}, got {actual}"
        )))
    }
}

pub(crate) fn map_status(code: i32, context: &str) -> BridgeResult<()> {
    if code == LATTICE_AUDIO_OK {
        Ok(())
    } else {
        Err(map_code(code, context))
    }
}

pub(crate) fn map_code(code: i32, context: &str) -> CaptureError {
    match code {
        LATTICE_AUDIO_ERR_INVALID_ARG => {
            CaptureError::invalid_argument(format!("{context}: invalid argument (code {code})"))
        }
        LATTICE_AUDIO_ERR_NOT_ARMED => CaptureError::NotArmed,
        LATTICE_AUDIO_ERR_ALREADY_RUNNING => CaptureError::AlreadyRunning,
        LATTICE_AUDIO_ERR_PERMISSION => CaptureError::PermissionDenied,
        LATTICE_AUDIO_ERR_DEVICE => CaptureError::device(format!("{context}: device error")),
        LATTICE_AUDIO_ERR_NOT_RUNNING => CaptureError::NotRunning,
        LATTICE_AUDIO_ERR_UNSUPPORTED => {
            CaptureError::Unsupported(format!("{context}: unsupported (code {code})"))
        }
        LATTICE_AUDIO_ERR_INTERNAL => {
            CaptureError::provider(format!("{context}: internal bridge error (code {code})"))
        }
        _ => CaptureError::provider(format!("{context}: unknown bridge error (code {code})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_mismatch_is_fail_closed() {
        let err = ensure_abi_version(1, 2).unwrap_err();
        assert!(matches!(err, CaptureError::Provider(_)));
        assert!(err.to_string().contains("ABI mismatch"));
    }

    #[test]
    fn maps_permission_denied() {
        let err = map_code(LATTICE_AUDIO_ERR_PERMISSION, "arm");
        assert_eq!(err, CaptureError::PermissionDenied);
    }
}
