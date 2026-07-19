//! Bindgen-free `extern "C"` bindings for `include/lattice_voice_bridge.h`.

#![cfg_attr(not(link_bridge), allow(dead_code))]

use std::ffi::c_void;
use std::os::raw::c_char;
#[cfg(link_bridge)]
use std::os::raw::c_float;

pub type LatticeVoiceEngine = u64;
pub type LatticeVoiceSession = u64;

pub const LATTICE_VOICE_OK: i32 = 0;
pub const LATTICE_VOICE_ERR_INVALID_ARG: i32 = -1;
pub const LATTICE_VOICE_ERR_NOT_PREPARED: i32 = -2;
pub const LATTICE_VOICE_ERR_ALREADY_PREPARED: i32 = -3;
pub const LATTICE_VOICE_ERR_SESSION: i32 = -4;
pub const LATTICE_VOICE_ERR_CANCELLED: i32 = -5;
pub const LATTICE_VOICE_ERR_INTERNAL: i32 = -6;
pub const LATTICE_VOICE_ERR_UNSUPPORTED: i32 = -7;
pub const LATTICE_VOICE_ERR_NOT_FOUND: i32 = -8;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeVoiceEventKind {
    Partial = 1,
    Stable = 2,
    Final = 3,
    Error = 4,
    SpeechStarted = 5,
    Endpoint = 6,
}

#[repr(C)]
#[derive(Debug)]
pub struct LatticeVoiceEvent {
    pub kind: u32,
    pub text: *const c_char,
    pub text_len: u32,
    pub stable_prefix_bytes: u32,
    pub error_code: i32,
}

pub type LatticeVoiceEventCallback =
    Option<unsafe extern "C" fn(*const LatticeVoiceEvent, *mut c_void)>;

#[cfg(link_bridge)]
#[link(name = "LatticeVoiceBridge", kind = "dylib")]
extern "C" {
    pub fn lattice_voice_bridge_abi_version() -> u32;

    pub fn lattice_voice_engine_create(
        model_cache_dir: *const c_char,
        out_engine: *mut LatticeVoiceEngine,
    ) -> i32;

    pub fn lattice_voice_engine_prepare(engine: LatticeVoiceEngine) -> i32;

    pub fn lattice_voice_engine_destroy(engine: LatticeVoiceEngine);

    pub fn lattice_voice_session_start(
        engine: LatticeVoiceEngine,
        callback: LatticeVoiceEventCallback,
        context: *mut c_void,
        out_session: *mut LatticeVoiceSession,
    ) -> i32;

    pub fn lattice_voice_session_push_audio(
        session: LatticeVoiceSession,
        samples: *const c_float,
        sample_count: usize,
    ) -> i32;

    pub fn lattice_voice_session_finish_utterance(session: LatticeVoiceSession) -> i32;

    pub fn lattice_voice_session_cancel(session: LatticeVoiceSession) -> i32;

    pub fn lattice_voice_session_destroy(session: LatticeVoiceSession);
}

/// Copy transcript bytes from a bridge callback event.
///
/// Pointers are only valid for the duration of the callback.
pub(crate) fn copy_event_text(event: &LatticeVoiceEvent) -> String {
    if event.text.is_null() || event.text_len == 0 {
        return String::new();
    }

    let len = event.text_len as usize;
    let bytes = unsafe { std::slice::from_raw_parts(event.text as *const u8, len) };
    String::from_utf8_lossy(bytes).into_owned()
}
