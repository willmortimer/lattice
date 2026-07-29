//! Secure Enclave approval signing bridge (macOS CryptoKit via Swift dylib).

mod ffi;

use std::ffi::CStr;
use std::sync::OnceLock;

/// ABI expected from `libLatticeApprovalBridge.dylib`.
pub const APPROVAL_BRIDGE_ABI_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct SeApprovalSigner {
    device_id: String,
    key_id: String,
}

impl SeApprovalSigner {
    pub fn load_or_create() -> Result<Self, String> {
        ensure_linked()?;
        let rc = unsafe { ffi::lattice_approval_load_or_create() };
        if rc != 0 {
            return Err(format!(
                "lattice_approval_load_or_create failed (code {rc})"
            ));
        }
        let device_id = read_string(ffi::lattice_approval_device_id)?;
        let key_id = read_string(ffi::lattice_approval_key_id)?;
        Ok(Self { device_id, key_id })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn backend(&self) -> &'static str {
        "secure-enclave"
    }

    pub fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        let mut out_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let rc = unsafe {
            ffi::lattice_approval_sign(
                payload.as_ptr(),
                payload.len(),
                &mut out_ptr,
                &mut out_len,
            )
        };
        if rc != 0 {
            return Err(format!("lattice_approval_sign failed (code {rc})"));
        }
        if out_ptr.is_null() || out_len == 0 {
            return Err("empty SE signature".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
        unsafe { ffi::lattice_approval_buffer_free(out_ptr, out_len) };
        Ok(bytes)
    }
}

fn ensure_linked() -> Result<(), String> {
    #[cfg(link_bridge)]
    {
        static ONCE: OnceLock<Result<(), String>> = OnceLock::new();
        ONCE.get_or_init(|| {
            let version = unsafe { ffi::lattice_approval_bridge_abi_version() };
            if version != APPROVAL_BRIDGE_ABI_VERSION {
                return Err(format!(
                    "LatticeApprovalBridge ABI mismatch: expected {APPROVAL_BRIDGE_ABI_VERSION}, got {version}"
                ));
            }
            Ok(())
        })
        .clone()
    }
    #[cfg(not(link_bridge))]
    {
        Err(
            "LatticeApprovalBridge not linked; build with --features link-bridge".into(),
        )
    }
}

fn read_string(
    f: unsafe extern "C" fn(*mut *mut std::os::raw::c_char) -> i32,
) -> Result<String, String> {
    let mut ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
    let rc = unsafe { f(&mut ptr) };
    if rc != 0 || ptr.is_null() {
        return Err(format!("approval string export failed (code {rc})"));
    }
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::lattice_approval_string_free(ptr) };
    Ok(s)
}

#[cfg(test)]
mod tests {
    #[test]
    fn abi_constant_is_stable() {
        assert_eq!(super::APPROVAL_BRIDGE_ABI_VERSION, 1);
    }
}
