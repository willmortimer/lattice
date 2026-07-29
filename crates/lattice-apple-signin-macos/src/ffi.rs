#![allow(dead_code)]

use std::os::raw::c_char;

#[cfg(link_bridge)]
unsafe extern "C" {
    pub fn lattice_apple_signin_bridge_abi_version() -> u32;
    pub fn lattice_apple_signin_request(
        nonce: *const c_char,
        out_token: *mut *mut c_char,
        out_error: *mut *mut c_char,
    ) -> i32;
    pub fn lattice_apple_signin_string_free(ptr: *mut c_char);
}

#[cfg(not(link_bridge))]
pub unsafe fn lattice_apple_signin_bridge_abi_version() -> u32 {
    0
}

#[cfg(not(link_bridge))]
pub unsafe fn lattice_apple_signin_request(
    _nonce: *const c_char,
    _out_token: *mut *mut c_char,
    _out_error: *mut *mut c_char,
) -> i32 {
    1
}

#[cfg(not(link_bridge))]
pub unsafe fn lattice_apple_signin_string_free(_ptr: *mut c_char) {}
