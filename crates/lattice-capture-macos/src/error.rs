//! Map bridge error codes to `lattice_capture_core::CaptureError`.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use lattice_capture_core::CaptureError;

use crate::ffi::{
    LATTICE_CAPTURE_ERR_CANCELLED, LATTICE_CAPTURE_ERR_INTERNAL, LATTICE_CAPTURE_ERR_INVALID_ARG,
    LATTICE_CAPTURE_ERR_NOT_FOUND, LATTICE_CAPTURE_ERR_NOT_IMPLEMENTED,
    LATTICE_CAPTURE_ERR_PERMISSION, LATTICE_CAPTURE_ERR_UNSUPPORTED, LATTICE_CAPTURE_OK,
};

pub(crate) type BridgeResult<T> = Result<T, CaptureError>;

/// Fail closed when the linked bridge reports an unexpected ABI version.
pub fn ensure_abi_version(expected: u32, actual: u32) -> Result<(), CaptureError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CaptureError::provider(format!(
            "LatticeCaptureBridge ABI mismatch: expected {expected}, got {actual}"
        )))
    }
}

pub(crate) fn map_status(code: i32, context: &str) -> BridgeResult<()> {
    if code == LATTICE_CAPTURE_OK {
        Ok(())
    } else {
        Err(map_code(code, context))
    }
}

pub(crate) fn map_code(code: i32, context: &str) -> CaptureError {
    match code {
        LATTICE_CAPTURE_ERR_INVALID_ARG => {
            CaptureError::invalid_argument(format!("{context}: invalid argument (code {code})"))
        }
        LATTICE_CAPTURE_ERR_CANCELLED => CaptureError::Cancelled,
        LATTICE_CAPTURE_ERR_PERMISSION => CaptureError::PermissionDenied,
        LATTICE_CAPTURE_ERR_NOT_FOUND => CaptureError::not_found(format!("{context}: not found")),
        LATTICE_CAPTURE_ERR_UNSUPPORTED => {
            CaptureError::Unsupported(format!("{context}: unsupported (code {code})"))
        }
        LATTICE_CAPTURE_ERR_NOT_IMPLEMENTED => CaptureError::Unsupported(format!(
            "{context}: not implemented (code {code})"
        )),
        LATTICE_CAPTURE_ERR_INTERNAL => {
            CaptureError::internal(format!("{context}: internal bridge error (code {code})"))
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
        let err = map_code(LATTICE_CAPTURE_ERR_PERMISSION, "capture_display");
        assert_eq!(err, CaptureError::PermissionDenied);
    }

    #[test]
    fn maps_cancelled() {
        let err = map_code(LATTICE_CAPTURE_ERR_CANCELLED, "interactive");
        assert_eq!(err, CaptureError::Cancelled);
    }
}
