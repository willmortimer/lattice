//! Sign in with Apple (AuthenticationServices) via Swift dylib.

mod ffi;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::OnceLock;

pub const APPLE_SIGNIN_BRIDGE_ABI_VERSION: u32 = 1;

/// Present the system Sign in with Apple sheet and return the identity JWT.
pub fn request_identity_token(nonce: Option<&str>) -> Result<String, String> {
    ensure_linked()?;
    let nonce_c = match nonce {
        Some(value) => Some(CString::new(value).map_err(|err| err.to_string())?),
        None => None,
    };
    let mut out_token: *mut c_char = ptr::null_mut();
    let mut out_error: *mut c_char = ptr::null_mut();
    let rc = unsafe {
        ffi::lattice_apple_signin_request(
            nonce_c
                .as_ref()
                .map(|value| value.as_ptr())
                .unwrap_or(ptr::null()),
            &mut out_token,
            &mut out_error,
        )
    };
    let error = take_string(out_error);
    if rc != 0 {
        return Err(error.unwrap_or_else(|| format!("Sign in with Apple failed (code {rc})")));
    }
    take_string(out_token).ok_or_else(|| "empty Apple identity token".into())
}

fn take_string(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::lattice_apple_signin_string_free(ptr) };
    Some(value)
}

fn ensure_linked() -> Result<(), String> {
    #[cfg(link_bridge)]
    {
        static ONCE: OnceLock<Result<(), String>> = OnceLock::new();
        ONCE.get_or_init(|| {
            let version = unsafe { ffi::lattice_apple_signin_bridge_abi_version() };
            if version != APPLE_SIGNIN_BRIDGE_ABI_VERSION {
                return Err(format!(
                    "LatticeAppleSignInBridge ABI mismatch: expected {APPLE_SIGNIN_BRIDGE_ABI_VERSION}, got {version}"
                ));
            }
            Ok(())
        })
        .clone()
    }
    #[cfg(not(link_bridge))]
    {
        let _ = OnceLock::<()>::new();
        Err(
            "Sign in with Apple bridge is not linked (build desktop with link-bridge)".into(),
        )
    }
}
