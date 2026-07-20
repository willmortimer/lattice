//! Voice dictation for the Tauri desktop shell.
//!
//! **Preferred (M4):** native capture in-process → packed PCM → `latticed` via
//! [`lattice_client::DaemonClient`] → `lattice-voice-host`.
//!
//! **Fallback (transition):** when latticed/voice-host is unavailable and the
//! `voice-embedded` feature is enabled, prepare in-process FluidAudio with a
//! clear "degraded embedded" log. Set `LATTICE_VOICE_DAEMON=1` to require the
//! daemon path (no FluidAudio symbols needed when building with `--features voice`
//! only).

#[cfg(all(target_os = "macos", feature = "voice"))]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(all(target_os = "macos", feature = "voice"))]
use lattice_client::LatticeClient;

use serde::{Deserialize, Serialize};
#[cfg(all(target_os = "macos", feature = "voice"))]
use tauri::Emitter;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

#[cfg(all(target_os = "macos", feature = "voice"))]
mod capture;
#[cfg(all(target_os = "macos", feature = "voice"))]
mod daemon;
#[cfg(all(target_os = "macos", feature = "voice-embedded"))]
mod embedded;

#[cfg(all(target_os = "macos", feature = "voice"))]
const VOICE_EVENT: &str = "voice-event";

#[cfg(all(target_os = "macos", feature = "voice"))]
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct VoiceState {
    // Locked for voice-feature paths; unused when `voice` is off (commands early-return).
    #[allow(dead_code)]
    inner: Mutex<VoiceInner>,
}

#[derive(Default)]
struct VoiceInner {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    preparing: bool,
    #[cfg(all(target_os = "macos", feature = "voice"))]
    daemon: Option<daemon::DaemonBackend>,
    #[cfg(all(target_os = "macos", feature = "voice-embedded"))]
    embedded: Option<embedded::EmbeddedBackend>,
    #[cfg(all(target_os = "macos", feature = "voice"))]
    active: Option<ActiveSession>,
    #[cfg(all(target_os = "macos", feature = "voice"))]
    capture: Option<lattice_audio_macos::MacOsCaptureProvider>,
    #[cfg(all(target_os = "macos", feature = "voice"))]
    capture_rx: Option<tokio::sync::mpsc::UnboundedReceiver<lattice_audio::CaptureEvent>>,
    #[cfg(all(target_os = "macos", feature = "voice"))]
    capture_armed: bool,
}

#[cfg(all(target_os = "macos", feature = "voice"))]
enum ActiveSession {
    Daemon(daemon::DaemonActiveSession),
    #[cfg(feature = "voice-embedded")]
    Embedded(embedded::EmbeddedActiveSession),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    pub available: bool,
    pub prepared: bool,
    pub preparing: bool,
    pub listening: bool,
    /// True when native (non-WebView) capture owns the microphone.
    pub native_capture: bool,
    pub platform: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSessionStart {
    pub session_id: String,
}

#[cfg(all(target_os = "macos", feature = "voice"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTranscriptCorrection {
    pub kind: String,
    pub raw_start: usize,
    pub raw_end: usize,
    pub replacement: String,
    pub source: String,
}

#[cfg(all(target_os = "macos", feature = "voice-embedded"))]
impl From<&lattice_voice::CorrectionProvenance> for VoiceTranscriptCorrection {
    fn from(value: &lattice_voice::CorrectionProvenance) -> Self {
        use lattice_voice::{CorrectionKind, CorrectionSource};
        let kind = match value.kind {
            CorrectionKind::SpokenPunctuation => "spoken_punctuation",
            CorrectionKind::PathReconstruction => "path_reconstruction",
            CorrectionKind::IdentifierCasing => "identifier_casing",
        };
        let source = match value.source {
            CorrectionSource::DeterministicRule => "deterministic_rule",
            CorrectionSource::GlossaryExactMatch => "glossary_exact_match",
            CorrectionSource::KnownPathMatch => "known_path_match",
        };
        Self {
            kind: kind.into(),
            raw_start: value.raw_start,
            raw_end: value.raw_end,
            replacement: value.replacement.clone(),
            source: source.into(),
        }
    }
}

#[cfg(all(target_os = "macos", feature = "voice"))]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoiceUiEvent {
    #[serde(rename_all = "camelCase")]
    Partial {
        session_id: String,
        revision: u64,
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Final {
        session_id: String,
        text: String,
        replaces_revision: Option<u64>,
        raw_text: Option<String>,
        corrections: Vec<VoiceTranscriptCorrection>,
    },
    #[serde(rename_all = "camelCase")]
    Status {
        state: String,
        message: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Failed {
        session_id: Option<String>,
        message: String,
    },
}

#[cfg(not(all(target_os = "macos", feature = "voice")))]
fn unsupported() -> String {
    "voice dictation requires macOS arm64 with `--features voice` (daemon thin client) \
     or `--features voice-embedded` (transitional FluidAudio fallback), plus native capture"
        .into()
}

#[cfg(all(target_os = "macos", feature = "voice"))]
async fn take_active_session(state: &VoiceState) -> Option<ActiveSession> {
    let mut inner = state.inner.lock().await;
    inner.active.take()
}

#[cfg(all(target_os = "macos", feature = "voice"))]
async fn shutdown_active_session_with_app(
    app: &AppHandle,
    active: ActiveSession,
    rearm: bool,
    state: &VoiceState,
) {
    match active {
        ActiveSession::Daemon(active) => {
            let mut inner = state.inner.lock().await;
            if rearm {
                let _ = daemon::cancel_session(app, active, &mut inner).await;
            } else {
                let daemon::DaemonActiveSession {
                    session_id,
                    client,
                    pump,
                    forwarder,
                    final_rx: _,
                } = active;
                pump.abort();
                forwarder.abort();
                let _ = client
                    .request(lattice_client::Request {
                        deadline_unix_ms: None,
                        idempotency_key: None,
                        body: Some(lattice_client::request::Body::CancelVoiceSession(
                            lattice_protocol::CancelVoiceSessionRequest {
                                session_id,
                                reason: Some("preempt".into()),
                            },
                        )),
                    })
                    .await;
            }
        }
        #[cfg(feature = "voice-embedded")]
        ActiveSession::Embedded(active) => {
            let mut inner = state.inner.lock().await;
            embedded::shutdown_active(active, rearm, &mut inner).await;
        }
    }
}

#[cfg(all(target_os = "macos", feature = "voice"))]
async fn ensure_voice_ready(app: &AppHandle, state: &VoiceState) -> Result<(), String> {
    {
        let inner = state.inner.lock().await;
        if inner.daemon.as_ref().is_some_and(|d| d.prepared) {
            return Ok(());
        }
        #[cfg(feature = "voice-embedded")]
        if inner.embedded.is_some() {
            return Ok(());
        }
        if inner.preparing {
            return Err(
                "voice model prepare is already in progress; wait for it to finish".into(),
            );
        }
    }

    {
        let mut inner = state.inner.lock().await;
        if inner.preparing {
            return Err(
                "voice model prepare is already in progress; wait for it to finish".into(),
            );
        }
        inner.preparing = true;
    }

    let daemon_result = async {
        let (client, child, fake_host) = daemon::connect_or_spawn().await?;
        let mut backend = daemon::DaemonBackend {
            client,
            _child: child,
            prepared: false,
            fake_host,
        };
        daemon::prepare(app, &mut backend).await?;
        Ok::<_, String>(backend)
    }
    .await;

    match daemon_result {
        Ok(backend) => {
            let mut inner = state.inner.lock().await;
            inner.preparing = false;
            if let Err(err) = capture::ensure_capture(&mut inner) {
                eprintln!("voice: capture arm after daemon prepare failed: {err}");
                return Err(format!("native capture unavailable: {err}"));
            }
            inner.daemon = Some(backend);
            Ok(())
        }
        Err(daemon_err) => {
            if daemon::daemon_required() {
                let mut inner = state.inner.lock().await;
                inner.preparing = false;
                return Err(format!(
                    "LATTICE_VOICE_DAEMON=1 requires latticed voice: {daemon_err}"
                ));
            }

            #[cfg(feature = "voice-embedded")]
            {
                eprintln!(
                    "voice: degraded embedded — daemon/voice-host unavailable ({daemon_err}); \
                     using in-process FluidAudio"
                );
                let mut inner = state.inner.lock().await;
                inner.preparing = false;
                let _ = embedded::ensure_provider(app, &mut inner).await?;
                return Ok(());
            }

            #[cfg(not(feature = "voice-embedded"))]
            {
                let mut inner = state.inner.lock().await;
                inner.preparing = false;
                Err(format!(
                    "latticed voice unavailable ({daemon_err}). \
                     Start latticed with voice-host \
                     (LATTICE_VOICE_FAKE=1 LATTICE_VOICE_HOST_BIN=…) \
                     or rebuild with `--features voice-embedded` for degraded fallback."
                ))
            }
        }
    }
}

#[tauri::command]
pub async fn voice_status(state: State<'_, VoiceState>) -> Result<VoiceStatus, String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        let inner = state.inner.lock().await;
        let prepared = inner.daemon.as_ref().is_some_and(|d| d.prepared)
            || {
                #[cfg(feature = "voice-embedded")]
                {
                    inner.embedded.is_some()
                }
                #[cfg(not(feature = "voice-embedded"))]
                {
                    false
                }
            };
        let message = if inner.preparing {
            Some("Preparing voice model…".into())
        } else if inner.daemon.as_ref().is_some_and(|d| d.prepared) {
            Some("Voice ready via latticed".into())
        } else {
            #[cfg(feature = "voice-embedded")]
            {
                if inner.embedded.is_some() {
                    Some("Voice ready (degraded embedded FluidAudio)".into())
                } else {
                    None
                }
            }
            #[cfg(not(feature = "voice-embedded"))]
            {
                None
            }
        };
        return Ok(VoiceStatus {
            available: true,
            prepared,
            preparing: inner.preparing,
            listening: inner.active.is_some(),
            native_capture: true,
            platform: "macos".into(),
            message,
        });
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = state;
        Ok(VoiceStatus {
            available: false,
            prepared: false,
            preparing: false,
            listening: false,
            native_capture: false,
            platform: std::env::consts::OS.into(),
            message: Some(unsupported()),
        })
    }
}

#[tauri::command]
pub async fn voice_prepare(
    app: AppHandle,
    state: State<'_, VoiceState>,
) -> Result<VoiceStatus, String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        ensure_voice_ready(&app, &state).await?;
        voice_status(state).await
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn voice_cancel_active(
    app: AppHandle,
    state: State<'_, VoiceState>,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        if let Some(active) = take_active_session(&state).await {
            shutdown_active_session_with_app(&app, active, true, &state).await;
        }
        let _ = app.emit(
            VOICE_EVENT,
            VoiceUiEvent::Status {
                state: "idle".into(),
                message: None,
            },
        );
        Ok(())
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceSessionContextHints {
    pub document_id: Option<String>,
    pub document_path: Option<String>,
    pub page_title: Option<String>,
    pub workspace_name: Option<String>,
    pub tags: Vec<String>,
    pub heading_path: Vec<String>,
    pub glossary_terms: Vec<String>,
    pub known_paths: Vec<String>,
}

impl Default for VoiceSessionContextHints {
    fn default() -> Self {
        Self {
            document_id: None,
            document_path: None,
            page_title: None,
            workspace_name: None,
            tags: Vec::new(),
            heading_path: Vec::new(),
            glossary_terms: Vec::new(),
            known_paths: Vec::new(),
        }
    }
}

#[tauri::command]
pub async fn voice_start_session(
    app: AppHandle,
    state: State<'_, VoiceState>,
    hints: Option<VoiceSessionContextHints>,
) -> Result<VoiceSessionStart, String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        if let Some(stale) = take_active_session(&state).await {
            shutdown_active_session_with_app(&app, stale, true, &state).await;
        }

        ensure_voice_ready(&app, &state).await?;

        let session_id = format!("voice-{}", NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
        let hints = hints.unwrap_or_default();

        let active = {
            let mut inner = state.inner.lock().await;
            if inner.daemon.as_ref().is_some_and(|d| d.prepared) {
                ActiveSession::Daemon(
                    daemon::start_session(app.clone(), &mut inner, session_id.clone(), hints)
                        .await?,
                )
            } else {
                #[cfg(feature = "voice-embedded")]
                {
                    ActiveSession::Embedded(
                        embedded::start_session(
                            app.clone(),
                            &mut inner,
                            session_id.clone(),
                            hints,
                        )
                        .await?,
                    )
                }
                #[cfg(not(feature = "voice-embedded"))]
                {
                    let _ = hints;
                    return Err("no voice backend prepared".into());
                }
            }
        };

        if let Some(stale) = take_active_session(&state).await {
            shutdown_active_session_with_app(&app, stale, false, &state).await;
        }
        {
            let mut inner = state.inner.lock().await;
            inner.active = Some(active);
        }

        let _ = app.emit(
            VOICE_EVENT,
            VoiceUiEvent::Status {
                state: "listening".into(),
                message: None,
            },
        );

        Ok(VoiceSessionStart { session_id })
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state, hints);
        Err(unsupported())
    }
}

/// Retired: WebView JSON float arrays are no longer accepted.
#[tauri::command]
pub async fn voice_push_audio(
    _state: State<'_, VoiceState>,
    _session_id: String,
    _samples: Vec<f32>,
) -> Result<(), String> {
    Err(
        "voice_push_audio is retired: macOS dictation uses native capture (no WebView PCM)"
            .into(),
    )
}

#[tauri::command]
pub async fn voice_finish_session(
    app: AppHandle,
    state: State<'_, VoiceState>,
    session_id: String,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        let active = {
            let mut inner = state.inner.lock().await;
            let active = inner
                .active
                .take()
                .ok_or_else(|| "no active dictation session".to_string())?;
            let matches = match &active {
                ActiveSession::Daemon(a) => a.session_id == session_id,
                #[cfg(feature = "voice-embedded")]
                ActiveSession::Embedded(a) => a.session_id == session_id,
            };
            if !matches {
                inner.active = Some(active);
                return Err("session id mismatch".into());
            }
            active
        };

        match active {
            ActiveSession::Daemon(active) => {
                let mut inner = state.inner.lock().await;
                daemon::finish_session(&app, active, &mut inner).await
            }
            #[cfg(feature = "voice-embedded")]
            ActiveSession::Embedded(active) => {
                let mut inner = state.inner.lock().await;
                embedded::finish_session(&app, active, &mut inner).await
            }
        }
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state, session_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn voice_cancel_session(
    app: AppHandle,
    state: State<'_, VoiceState>,
    session_id: String,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        let active = {
            let mut inner = state.inner.lock().await;
            let Some(active) = inner.active.take() else {
                return Ok(());
            };
            let matches = match &active {
                ActiveSession::Daemon(a) => a.session_id == session_id,
                #[cfg(feature = "voice-embedded")]
                ActiveSession::Embedded(a) => a.session_id == session_id,
            };
            if !matches {
                inner.active = Some(active);
                return Err("session id mismatch".into());
            }
            active
        };

        match active {
            ActiveSession::Daemon(active) => {
                let mut inner = state.inner.lock().await;
                daemon::cancel_session(&app, active, &mut inner).await
            }
            #[cfg(feature = "voice-embedded")]
            ActiveSession::Embedded(active) => {
                let mut inner = state.inner.lock().await;
                embedded::cancel_session(&app, active, &mut inner).await
            }
        }
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state, session_id);
        Err(unsupported())
    }
}
