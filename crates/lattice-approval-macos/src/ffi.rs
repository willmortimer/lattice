//! FFI declarations for LatticeApprovalBridge.

#![allow(dead_code)]

use std::os::raw::{c_char, c_int};

#[cfg(link_bridge)]
#[link(name = "LatticeApprovalBridge", kind = "dylib")]
extern "C" {
    pub fn lattice_approval_bridge_abi_version() -> u32;
    pub fn lattice_approval_load_or_create() -> c_int;
    pub fn lattice_approval_shutdown();
    pub fn lattice_approval_backend() -> *const c_char;
    pub fn lattice_approval_device_id(out: *mut *mut c_char) -> c_int;
    pub fn lattice_approval_key_id(out: *mut *mut c_char) -> c_int;
    pub fn lattice_approval_sign(
        payload: *const u8,
        payload_len: usize,
        out_sig: *mut *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    pub fn lattice_approval_string_free(ptr: *mut c_char);
    pub fn lattice_approval_buffer_free(ptr: *mut u8, len: usize);
}

#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_bridge_abi_version() -> u32 {
    0
}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_load_or_create() -> c_int {
    -1
}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_shutdown() {}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_backend() -> *const c_char {
    std::ptr::null()
}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_device_id(_out: *mut *mut c_char) -> c_int {
    -1
}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_key_id(_out: *mut *mut c_char) -> c_int {
    -1
}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_sign(
    _payload: *const u8,
    _payload_len: usize,
    _out_sig: *mut *mut u8,
    _out_len: *mut usize,
) -> c_int {
    -1
}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_string_free(_ptr: *mut c_char) {}
#[cfg(not(link_bridge))]
pub unsafe fn lattice_approval_buffer_free(_ptr: *mut u8, _len: usize) {}
