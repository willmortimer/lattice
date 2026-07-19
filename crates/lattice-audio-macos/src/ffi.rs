//! Bindgen-free `extern "C"` bindings for `include/lattice_audio_bridge.h`.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use std::ffi::c_void;
use std::os::raw::c_char;

pub type LatticeAudioCapture = u64;

pub const LATTICE_AUDIO_OK: i32 = 0;
pub const LATTICE_AUDIO_ERR_INVALID_ARG: i32 = -1;
pub const LATTICE_AUDIO_ERR_NOT_ARMED: i32 = -2;
pub const LATTICE_AUDIO_ERR_ALREADY_RUNNING: i32 = -3;
pub const LATTICE_AUDIO_ERR_PERMISSION: i32 = -4;
pub const LATTICE_AUDIO_ERR_DEVICE: i32 = -5;
pub const LATTICE_AUDIO_ERR_INTERNAL: i32 = -6;
pub const LATTICE_AUDIO_ERR_UNSUPPORTED: i32 = -7;
pub const LATTICE_AUDIO_ERR_NOT_RUNNING: i32 = -8;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeAudioEventKind {
    Started = 1,
    Frame = 2,
    Gap = 3,
    Stopped = 4,
    Error = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LatticeAudioFrame {
    pub sequence: u64,
    pub captured_at_ns: u64,
    pub frame_count: u32,
    pub samples: *const f32,
    pub peak_abs: f32,
    pub rms: f32,
    pub clipped: u8,
    pub _pad: [u8; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LatticeAudioGap {
    pub from_sequence: u64,
    pub to_sequence: u64,
    pub captured_at_ns: u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct LatticeAudioEvent {
    pub kind: u32,
    pub captured_at_ns: u64,
    pub frame: LatticeAudioFrame,
    pub gap: LatticeAudioGap,
    pub error_code: i32,
    pub error_message: *const c_char,
    pub error_message_len: u32,
}

pub type LatticeAudioEventCallback =
    Option<unsafe extern "C" fn(*const LatticeAudioEvent, *mut c_void)>;

#[cfg(link_bridge)]
#[link(name = "LatticeAudioBridge", kind = "dylib")]
extern "C" {
    pub fn lattice_audio_bridge_abi_version() -> u32;

    pub fn lattice_audio_capture_create(
        pre_roll_ms: u32,
        enable_diagnostics: u8,
        out_capture: *mut LatticeAudioCapture,
    ) -> i32;

    pub fn lattice_audio_capture_arm(capture: LatticeAudioCapture) -> i32;

    pub fn lattice_audio_capture_start(
        capture: LatticeAudioCapture,
        callback: LatticeAudioEventCallback,
        context: *mut c_void,
    ) -> i32;

    pub fn lattice_audio_capture_stop(capture: LatticeAudioCapture) -> i32;

    pub fn lattice_audio_capture_destroy(capture: LatticeAudioCapture);
}

/// Copy Float32 samples from a bridge frame callback.
///
/// Pointers are only valid for the duration of the callback.
pub(crate) fn copy_frame_samples(frame: &LatticeAudioFrame) -> Vec<f32> {
    if frame.samples.is_null() || frame.frame_count == 0 {
        return Vec::new();
    }
    let len = frame.frame_count as usize;
    unsafe { std::slice::from_raw_parts(frame.samples, len) }.to_vec()
}

/// Copy error message bytes from a bridge callback event.
pub(crate) fn copy_error_message(event: &LatticeAudioEvent) -> String {
    if event.error_message.is_null() || event.error_message_len == 0 {
        return String::new();
    }
    let len = event.error_message_len as usize;
    let bytes = unsafe { std::slice::from_raw_parts(event.error_message as *const u8, len) };
    String::from_utf8_lossy(bytes).into_owned()
}