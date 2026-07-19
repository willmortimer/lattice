//! Native and test-double backends behind the C ABI.

use std::ffi::c_void;
use std::ffi::CString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(test)]
use std::sync::Mutex;

use crate::error::BridgeResult;
#[cfg(link_bridge)]
use crate::error::{ensure_abi_version, map_status};
#[cfg(any(link_bridge, test))]
use crate::LATTICE_VOICE_BRIDGE_ABI_VERSION;
use crate::ffi::{
    copy_event_text, LatticeVoiceEngine, LatticeVoiceEvent, LatticeVoiceEventCallback,
    LatticeVoiceEventKind, LatticeVoiceSession,
};
#[cfg(test)]
use crate::ffi::LATTICE_VOICE_OK;
#[cfg(any(test, not(link_bridge)))]
use lattice_voice::SpeechError;

/// Owned bridge event copied off the FluidAudio callback thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedBridgeEvent {
    pub kind: LatticeVoiceEventKind,
    pub text: String,
    pub stable_prefix_bytes: u32,
    pub error_code: i32,
}

impl OwnedBridgeEvent {
    pub(crate) fn from_raw(event: &LatticeVoiceEvent) -> Self {
        let kind = match event.kind {
            1 => LatticeVoiceEventKind::Partial,
            2 => LatticeVoiceEventKind::Stable,
            3 => LatticeVoiceEventKind::Final,
            4 => LatticeVoiceEventKind::Error,
            5 => LatticeVoiceEventKind::SpeechStarted,
            6 => LatticeVoiceEventKind::Endpoint,
            _ => LatticeVoiceEventKind::Error,
        };

        Self {
            kind,
            text: copy_event_text(event),
            stable_prefix_bytes: event.stable_prefix_bytes,
            error_code: event.error_code,
        }
    }
}

/// Callback context passed through the C ABI as `void *`.
pub(crate) struct CallbackContext {
    pub(crate) tx: std::sync::mpsc::Sender<OwnedBridgeEvent>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

/// Raw pointer wrapper — the context is only accessed on the callback thread
/// and in `Drop` on the session owner thread.
pub(crate) struct CallbackContextPtr(*mut CallbackContext);

unsafe impl Send for CallbackContextPtr {}
unsafe impl Sync for CallbackContextPtr {}

impl CallbackContextPtr {
    pub(crate) fn new(ptr: *mut CallbackContext) -> Self {
        Self(ptr)
    }
}

impl Drop for CallbackContextPtr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                drop(Box::from_raw(self.0));
            }
        }
    }
}

pub(crate) unsafe extern "C" fn bridge_event_callback(
    event: *const LatticeVoiceEvent,
    context: *mut c_void,
) {
    if event.is_null() || context.is_null() {
        return;
    }

    let ctx = &*(context as *const CallbackContext);
    if ctx.cancelled.load(Ordering::Acquire) {
        return;
    }

    let owned = OwnedBridgeEvent::from_raw(&*event);
    let _ = ctx.tx.send(owned);
}

/// Engine/session operations used by the provider (native or mock).
pub(crate) trait VoiceBridgeBackend: Send + Sync {
    fn abi_version(&self) -> u32;
    fn engine_create(&self, model_cache_dir: Option<&Path>) -> BridgeResult<LatticeVoiceEngine>;
    fn engine_prepare(&self, engine: LatticeVoiceEngine) -> BridgeResult<()>;
    fn engine_destroy(&self, engine: LatticeVoiceEngine);
    fn session_start(
        &self,
        engine: LatticeVoiceEngine,
        callback: LatticeVoiceEventCallback,
        context: *mut c_void,
    ) -> BridgeResult<LatticeVoiceSession>;
    fn session_push_audio(
        &self,
        session: LatticeVoiceSession,
        samples: &[f32],
    ) -> BridgeResult<()>;
    fn session_finish_utterance(&self, session: LatticeVoiceSession) -> BridgeResult<()>;
    fn session_cancel(&self, session: LatticeVoiceSession) -> BridgeResult<()>;
    fn session_destroy(&self, session: LatticeVoiceSession);
}

/// Linked `libLatticeVoiceBridge.dylib` backend.
#[cfg(link_bridge)]
pub(crate) struct NativeBridge;

#[cfg(link_bridge)]
impl NativeBridge {
    pub(crate) fn new() -> BridgeResult<Self> {
        let actual = unsafe { crate::ffi::lattice_voice_bridge_abi_version() };
        ensure_abi_version(LATTICE_VOICE_BRIDGE_ABI_VERSION, actual)?;
        Ok(Self)
    }
}

#[cfg(link_bridge)]
impl VoiceBridgeBackend for NativeBridge {
    fn abi_version(&self) -> u32 {
        unsafe { crate::ffi::lattice_voice_bridge_abi_version() }
    }

    fn engine_create(&self, model_cache_dir: Option<&Path>) -> BridgeResult<LatticeVoiceEngine> {
        let c_path = model_cache_dir.and_then(|path| CString::new(path.to_string_lossy().as_ref()).ok());
        let mut engine = 0_u64;
        let status = unsafe {
            crate::ffi::lattice_voice_engine_create(
                c_path
                    .as_ref()
                    .map_or(std::ptr::null(), |value| value.as_ptr()),
                &mut engine,
            )
        };
        map_status(status, "lattice_voice_engine_create")?;
        Ok(engine)
    }

    fn engine_prepare(&self, engine: LatticeVoiceEngine) -> BridgeResult<()> {
        let status = unsafe { crate::ffi::lattice_voice_engine_prepare(engine) };
        map_status(status, "lattice_voice_engine_prepare")
    }

    fn engine_destroy(&self, engine: LatticeVoiceEngine) {
        unsafe { crate::ffi::lattice_voice_engine_destroy(engine) }
    }

    fn session_start(
        &self,
        engine: LatticeVoiceEngine,
        callback: LatticeVoiceEventCallback,
        context: *mut c_void,
    ) -> BridgeResult<LatticeVoiceSession> {
        let mut session = 0_u64;
        let status = unsafe {
            crate::ffi::lattice_voice_session_start(engine, callback, context, &mut session)
        };
        map_status(status, "lattice_voice_session_start")?;
        Ok(session)
    }

    fn session_push_audio(
        &self,
        session: LatticeVoiceSession,
        samples: &[f32],
    ) -> BridgeResult<()> {
        let status = unsafe {
            crate::ffi::lattice_voice_session_push_audio(
                session,
                samples.as_ptr(),
                samples.len(),
            )
        };
        map_status(status, "lattice_voice_session_push_audio")
    }

    fn session_finish_utterance(&self, session: LatticeVoiceSession) -> BridgeResult<()> {
        let status = unsafe { crate::ffi::lattice_voice_session_finish_utterance(session) };
        map_status(status, "lattice_voice_session_finish_utterance")
    }

    fn session_cancel(&self, session: LatticeVoiceSession) -> BridgeResult<()> {
        let status = unsafe { crate::ffi::lattice_voice_session_cancel(session) };
        map_status(status, "lattice_voice_session_cancel")
    }

    fn session_destroy(&self, session: LatticeVoiceSession) {
        unsafe { crate::ffi::lattice_voice_session_destroy(session) }
    }
}

/// In-memory backend for unit tests (no native library required).
#[cfg(test)]
pub(crate) struct MockBridge {
    abi_version: u32,
    engines: Mutex<Vec<MockEngine>>,
    sessions: Mutex<Vec<MockSession>>,
}

#[cfg(test)]
struct MockEngine {
    id: LatticeVoiceEngine,
    prepared: bool,
    destroyed: bool,
}

#[cfg(test)]
struct MockSession {
    id: LatticeVoiceSession,
    engine: LatticeVoiceEngine,
    callback: LatticeVoiceEventCallback,
    context: usize,
    cancelled: Arc<AtomicBool>,
    destroyed: bool,
}

#[cfg(test)]
impl MockBridge {
    pub(crate) fn new(abi_version: u32) -> Self {
        Self {
            abi_version,
            engines: Mutex::new(Vec::new()),
            sessions: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn session_handle(&self) -> Option<LatticeVoiceSession> {
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.first().map(|session| session.id))
    }

    pub(crate) fn emit_late_partial(&self, text: &str) -> bool {
        let Ok(sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(session) = sessions.first() else {
            return false;
        };
        if session.cancelled.load(Ordering::Acquire) || session.destroyed {
            return false;
        }
        Self::fire_callback(
            session.callback,
            session.context,
            LatticeVoiceEventKind::Partial,
            text,
            0,
            LATTICE_VOICE_OK,
        )
    }

    pub(crate) fn emit_speech_started(&self) -> bool {
        let Ok(sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(session) = sessions.first() else {
            return false;
        };
        if session.cancelled.load(Ordering::Acquire) || session.destroyed {
            return false;
        }
        Self::fire_callback(
            session.callback,
            session.context,
            LatticeVoiceEventKind::SpeechStarted,
            "",
            0,
            LATTICE_VOICE_OK,
        )
    }

    pub(crate) fn emit_endpoint(&self, reason_code: i32) -> bool {
        let Ok(sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(session) = sessions.first() else {
            return false;
        };
        if session.cancelled.load(Ordering::Acquire) || session.destroyed {
            return false;
        }
        Self::fire_callback(
            session.callback,
            session.context,
            LatticeVoiceEventKind::Endpoint,
            "",
            0,
            reason_code,
        )
    }

    fn fire_callback(
        callback: LatticeVoiceEventCallback,
        context: usize,
        kind: LatticeVoiceEventKind,
        text: &str,
        stable_prefix_bytes: u32,
        error_code: i32,
    ) -> bool {
        let Some(callback) = callback else {
            return false;
        };
        let Ok(c_text) = CString::new(text) else {
            return false;
        };
        let mut event = LatticeVoiceEvent {
            kind: kind as u32,
            text: c_text.as_ptr(),
            text_len: text.len() as u32,
            stable_prefix_bytes,
            error_code,
        };
        unsafe {
            callback(&mut event, context as *mut c_void);
        }
        true
    }
}

#[cfg(test)]
impl VoiceBridgeBackend for MockBridge {
    fn abi_version(&self) -> u32 {
        self.abi_version
    }

    fn engine_create(&self, _model_cache_dir: Option<&Path>) -> BridgeResult<LatticeVoiceEngine> {
        let mut engines = self
            .engines
            .lock()
            .map_err(|_| SpeechError::provider("mock engine lock poisoned"))?;
        let id = engines.len() as u64 + 1;
        engines.push(MockEngine {
            id,
            prepared: false,
            destroyed: false,
        });
        Ok(id)
    }

    fn engine_prepare(&self, engine: LatticeVoiceEngine) -> BridgeResult<()> {
        let mut engines = self
            .engines
            .lock()
            .map_err(|_| SpeechError::provider("mock engine lock poisoned"))?;
        let entry = engines
            .iter_mut()
            .find(|entry| entry.id == engine && !entry.destroyed)
            .ok_or_else(|| SpeechError::provider("unknown mock engine"))?;
        if entry.prepared {
            return Err(SpeechError::provider("engine is already prepared"));
        }
        entry.prepared = true;
        Ok(())
    }

    fn engine_destroy(&self, engine: LatticeVoiceEngine) {
        if let Ok(mut engines) = self.engines.lock() {
            if let Some(entry) = engines.iter_mut().find(|entry| entry.id == engine) {
                entry.destroyed = true;
            }
        }
    }

    fn session_start(
        &self,
        engine: LatticeVoiceEngine,
        callback: LatticeVoiceEventCallback,
        context: *mut c_void,
    ) -> BridgeResult<LatticeVoiceSession> {
        {
            let engines = self
                .engines
                .lock()
                .map_err(|_| SpeechError::provider("mock engine lock poisoned"))?;
            let entry = engines
                .iter()
                .find(|entry| entry.id == engine && !entry.destroyed)
                .ok_or_else(|| SpeechError::provider("unknown mock engine"))?;
            if !entry.prepared {
                return Err(SpeechError::provider("engine is not prepared"));
            }
        }

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| SpeechError::provider("mock session lock poisoned"))?;
        let id = sessions.len() as u64 + 1;
        sessions.push(MockSession {
            id,
            engine,
            callback,
            context: context as usize,
            cancelled: Arc::new(AtomicBool::new(false)),
            destroyed: false,
        });
        Ok(id)
    }

    fn session_push_audio(
        &self,
        session: LatticeVoiceSession,
        samples: &[f32],
    ) -> BridgeResult<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| SpeechError::provider("mock session lock poisoned"))?;
        let entry = sessions
            .iter()
            .find(|entry| entry.id == session && !entry.destroyed)
            .ok_or_else(|| SpeechError::provider("unknown mock session"))?;
        if entry.cancelled.load(Ordering::Acquire) {
            return Err(SpeechError::provider("session was cancelled"));
        }
        Self::fire_callback(
            entry.callback,
            entry.context,
            LatticeVoiceEventKind::Partial,
            "mock partial",
            0,
            LATTICE_VOICE_OK,
        );
        Ok(())
    }

    fn session_finish_utterance(&self, session: LatticeVoiceSession) -> BridgeResult<()> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| SpeechError::provider("mock session lock poisoned"))?;
        let entry = sessions
            .iter()
            .find(|entry| entry.id == session && !entry.destroyed)
            .ok_or_else(|| SpeechError::provider("unknown mock session"))?;
        if entry.cancelled.load(Ordering::Acquire) {
            return Err(SpeechError::provider("session was cancelled"));
        }
        Self::fire_callback(
            entry.callback,
            entry.context,
            LatticeVoiceEventKind::Final,
            "mock final",
            10,
            LATTICE_VOICE_OK,
        );
        Ok(())
    }

    fn session_cancel(&self, session: LatticeVoiceSession) -> BridgeResult<()> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| SpeechError::provider("mock session lock poisoned"))?;
        let entry = sessions
            .iter_mut()
            .find(|entry| entry.id == session && !entry.destroyed)
            .ok_or_else(|| SpeechError::provider("unknown mock session"))?;
        entry.cancelled.store(true, Ordering::Release);
        Ok(())
    }

    fn session_destroy(&self, session: LatticeVoiceSession) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(entry) = sessions.iter_mut().find(|entry| entry.id == session) {
                entry.destroyed = true;
            }
        }
    }
}

/// Construct the platform backend when the native library is linked.
#[cfg(link_bridge)]
pub(crate) fn new_backend() -> BridgeResult<Arc<dyn VoiceBridgeBackend>> {
    Ok(Arc::new(NativeBridge::new()?))
}

#[cfg(not(link_bridge))]
pub(crate) fn new_backend() -> BridgeResult<Arc<dyn VoiceBridgeBackend>> {
    Err(SpeechError::provider(
        "LatticeVoiceBridge is not linked; build the Swift dylib or set LATTICE_VOICE_BRIDGE_LIB",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn mock_create_destroy_engine_ordering() {
        let bridge = MockBridge::new(LATTICE_VOICE_BRIDGE_ABI_VERSION);
        let engine = bridge.engine_create(None).unwrap();
        bridge.engine_prepare(engine).unwrap();
        bridge.engine_destroy(engine);
    }

    #[test]
    fn mock_cancel_drops_late_callbacks() {
        let bridge = Arc::new(MockBridge::new(LATTICE_VOICE_BRIDGE_ABI_VERSION));
        let engine = bridge.engine_create(None).unwrap();
        bridge.engine_prepare(engine).unwrap();

        let (tx, rx) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let ctx = Box::new(CallbackContext {
            tx,
            cancelled: cancelled.clone(),
        });
        let ctx_ptr = Box::into_raw(ctx);

        let session = bridge
            .session_start(engine, Some(bridge_event_callback), ctx_ptr.cast())
            .unwrap();

        bridge.session_cancel(session).unwrap();
        cancelled.store(true, Ordering::Release);

        assert!(!bridge.emit_late_partial("late"));
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());

        bridge.session_destroy(session);
        bridge.engine_destroy(engine);
        unsafe {
            drop(Box::from_raw(ctx_ptr));
        }
    }
}
