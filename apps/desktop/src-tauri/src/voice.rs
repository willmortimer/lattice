//! In-process voice dictation for the Tauri desktop shell.
//!
//! On macOS with `--features voice`, microphone capture is owned by
//! `lattice-audio-macos` (AVAudioEngine → 16 kHz mono Float32). Packed frames
//! with sequence + `captured_at_ns` are pushed in-process to
//! `FluidAudioSpeechProvider`. The WebView only drives session intent.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

const VOICE_EVENT: &str = "voice-event";

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct VoiceState {
    inner: Mutex<VoiceInner>,
}

#[derive(Default)]
struct VoiceInner {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    provider: Option<Arc<lattice_voice_macos::FluidAudioSpeechProvider>>,
    #[cfg(all(target_os = "macos", feature = "voice"))]
    preparing: bool,
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
struct ActiveSession {
    session_id: String,
    normalization_context: lattice_voice::NormalizationContext,
    /// Shared with the capture pump until finish/cancel takes ownership.
    session: Arc<Mutex<Option<Box<dyn lattice_voice::SpeechSession>>>>,
    _forwarder: tokio::task::JoinHandle<()>,
    pump: tokio::task::JoinHandle<()>,
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
        corrections: Vec<lattice_voice::CorrectionProvenance>,
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
    "voice dictation requires macOS arm64 with `--features voice`, FluidAudio, and native capture"
        .into()
}

#[cfg(all(target_os = "macos", feature = "voice"))]
fn chunk_from_frame(
    session_id: &str,
    frame: &lattice_audio::AudioFrame,
) -> lattice_voice::AudioChunk {
    use lattice_voice::{AudioChunk, AudioSampleFormat};

    AudioChunk {
        session_id: session_id.to_string(),
        sequence: frame.sequence,
        captured_at_ns: frame.captured_at_ns,
        sample_rate_hz: frame.format.sample_rate_hz,
        channels: frame.format.channels,
        sample_format: AudioSampleFormat::F32,
        payload: frame.payload.clone(),
    }
}

#[cfg(all(target_os = "macos", feature = "voice"))]
fn ensure_capture(inner: &mut VoiceInner) -> Result<(), String> {
    use lattice_audio::CaptureProvider;
    use lattice_audio_macos::MacOsCaptureProvider;

    if inner.capture.is_none() {
        let mut provider = MacOsCaptureProvider::new();
        let rx = provider.subscribe();
        inner.capture = Some(provider);
        inner.capture_rx = Some(rx);
        inner.capture_armed = false;
    }

    if !inner.capture_armed {
        let capture = inner
            .capture
            .as_mut()
            .expect("capture created above");
        capture.arm().map_err(|err| err.to_string())?;
        inner.capture_armed = true;
    }

    Ok(())
}

#[cfg(all(target_os = "macos", feature = "voice"))]
fn stop_capture_and_rearm(inner: &mut VoiceInner) {
    use lattice_audio::CaptureProvider;

    let Some(capture) = inner.capture.as_mut() else {
        return;
    };
    let _ = capture.stop();
    inner.capture_armed = false;
    // Fresh subscriber for the next arm/start cycle.
    let rx = capture.subscribe();
    inner.capture_rx = Some(rx);
    match capture.arm() {
        Ok(()) => inner.capture_armed = true,
        Err(err) => {
            eprintln!("voice: failed to re-arm capture after stop: {err}");
        }
    }
}

#[cfg(all(target_os = "macos", feature = "voice"))]
async fn pump_capture_to_session(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<lattice_audio::CaptureEvent>,
    session: Arc<Mutex<Option<Box<dyn lattice_voice::SpeechSession>>>>,
    session_id: String,
    app: AppHandle,
) {
    use lattice_audio::{BoundedFrameQueue, CaptureEvent};

    let mut queue = BoundedFrameQueue::canonical_default();
    let mut expected_sequence = 0_u64;

    while let Some(event) = rx.recv().await {
        match event {
            CaptureEvent::Started { .. } => {}
            CaptureEvent::Frame(frame) => {
                if frame.sequence != expected_sequence {
                    let message = format!(
                        "capture sequence gap: expected {expected_sequence}, got {}",
                        frame.sequence
                    );
                    eprintln!("voice: {message}");
                    let _ = app.emit(
                        VOICE_EVENT,
                        VoiceUiEvent::Failed {
                            session_id: Some(session_id.clone()),
                            message,
                        },
                    );
                    return;
                }
                expected_sequence = expected_sequence.saturating_add(1);

                if let Err(gap) = queue.try_push(frame) {
                    let message = format!(
                        "audio backpressure gap: dropped sequence near {} (missing {})",
                        gap.to_sequence.saturating_sub(1),
                        gap.missing_count()
                    );
                    eprintln!("voice: {message}");
                    let _ = app.emit(
                        VOICE_EVENT,
                        VoiceUiEvent::Failed {
                            session_id: Some(session_id.clone()),
                            message,
                        },
                    );
                    return;
                }

                while let Some(frame) = queue.pop_front() {
                    let chunk = chunk_from_frame(&session_id, &frame);
                    let mut guard = session.lock().await;
                    let Some(speech) = guard.as_mut() else {
                        return;
                    };
                    if let Err(err) = speech.push_audio(chunk).await {
                        let message = err.to_string();
                        eprintln!("voice: push_audio failed: {message}");
                        let _ = app.emit(
                            VOICE_EVENT,
                            VoiceUiEvent::Failed {
                                session_id: Some(session_id.clone()),
                                message,
                            },
                        );
                        return;
                    }
                }
            }
            CaptureEvent::Gap(gap) => {
                let message = format!(
                    "native capture gap: from {} to {} (missing {})",
                    gap.from_sequence,
                    gap.to_sequence,
                    gap.missing_count()
                );
                eprintln!("voice: {message}");
                expected_sequence = gap.to_sequence;
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Status {
                        state: "listening".into(),
                        message: Some(message),
                    },
                );
            }
            CaptureEvent::Stopped { .. } => break,
            CaptureEvent::Error {
                message,
                captured_at_ns: _,
            } => {
                eprintln!("voice: capture error: {message}");
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Failed {
                        session_id: Some(session_id.clone()),
                        message,
                    },
                );
                break;
            }
        }
    }
}

#[tauri::command]
pub async fn voice_status(state: State<'_, VoiceState>) -> Result<VoiceStatus, String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        let inner = state.inner.lock().await;
        return Ok(VoiceStatus {
            available: true,
            prepared: inner.provider.is_some(),
            preparing: inner.preparing,
            listening: inner.active.is_some(),
            native_capture: true,
            platform: "macos".into(),
            message: if inner.preparing {
                Some(
                    "Preparing local voice model (first run downloads + compiles; may take several minutes)…"
                        .into(),
                )
            } else {
                None
            },
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

/// Load (and warm) the FluidAudio provider without holding the voice mutex
/// across the long download / Core ML compile.
#[cfg(all(target_os = "macos", feature = "voice"))]
async fn ensure_provider(
    app: &AppHandle,
    state: &VoiceState,
) -> Result<Arc<lattice_voice_macos::FluidAudioSpeechProvider>, String> {
    use lattice_voice::{PrepareModelRequest, SpeechProvider};
    use lattice_voice_macos::FluidAudioSpeechProvider;

    {
        let inner = state.inner.lock().await;
        if let Some(provider) = &inner.provider {
            return Ok(Arc::clone(provider));
        }
        if inner.preparing {
            return Err(
                "voice model prepare is already in progress; wait for it to finish".into(),
            );
        }
    }

    let provider = {
        let mut inner = state.inner.lock().await;
        if let Some(provider) = &inner.provider {
            return Ok(Arc::clone(provider));
        }
        if inner.preparing {
            return Err(
                "voice model prepare is already in progress; wait for it to finish".into(),
            );
        }
        inner.preparing = true;
        match FluidAudioSpeechProvider::new() {
            Ok(provider) => Arc::new(provider),
            Err(err) => {
                inner.preparing = false;
                return Err(err.to_string());
            }
        }
    };

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Status {
            state: "preparing".into(),
            message: Some(
                "Preparing local voice model (first run downloads + compiles; may take several minutes)…"
                    .into(),
            ),
        },
    );

    let prepare_result = provider
        .prepare(PrepareModelRequest {
            model_id: "parakeet-unified-320ms".into(),
            warm: true,
        })
        .await;

    let mut inner = state.inner.lock().await;
    inner.preparing = false;

    match prepare_result {
        Ok(_) => {
            inner.provider = Some(Arc::clone(&provider));
            // Arm native capture so pre-roll fills before the next PTT press.
            if let Err(err) = ensure_capture(&mut inner) {
                eprintln!("voice: capture arm after prepare failed: {err}");
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Failed {
                        session_id: None,
                        message: format!("native capture unavailable: {err}"),
                    },
                );
                return Err(format!("native capture unavailable: {err}"));
            }
            let _ = app.emit(
                VOICE_EVENT,
                VoiceUiEvent::Status {
                    state: "ready".into(),
                    message: Some("Local voice model ready".into()),
                },
            );
            Ok(provider)
        }
        Err(err) => {
            let message = err.to_string();
            let _ = app.emit(
                VOICE_EVENT,
                VoiceUiEvent::Failed {
                    session_id: None,
                    message: message.clone(),
                },
            );
            let _ = app.emit(
                VOICE_EVENT,
                VoiceUiEvent::Status {
                    state: "idle".into(),
                    message: Some(message.clone()),
                },
            );
            Err(message)
        }
    }
}

#[cfg(all(target_os = "macos", feature = "voice"))]
async fn take_active_session(state: &VoiceState) -> Option<ActiveSession> {
    let mut inner = state.inner.lock().await;
    inner.active.take()
}

#[cfg(all(target_os = "macos", feature = "voice"))]
async fn shutdown_active_session(active: ActiveSession, rearm: bool, state: &VoiceState) {
    active.pump.abort();
    {
        let mut guard = active.session.lock().await;
        if let Some(session) = guard.take() {
            let _ = session.cancel().await;
        }
    }
    if rearm {
        let mut inner = state.inner.lock().await;
        stop_capture_and_rearm(&mut inner);
    }
}

#[tauri::command]
pub async fn voice_prepare(
    app: AppHandle,
    state: State<'_, VoiceState>,
) -> Result<VoiceStatus, String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        ensure_provider(&app, &state).await?;
        let inner = state.inner.lock().await;
        Ok(VoiceStatus {
            available: true,
            prepared: true,
            preparing: false,
            listening: inner.active.is_some(),
            native_capture: true,
            platform: "macos".into(),
            message: Some("Local voice model ready".into()),
        })
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state);
        Err(unsupported())
    }
}

/// Cancel whatever session is active (id optional). Used when the UI releases
/// during startup before it has a session id.
#[tauri::command]
pub async fn voice_cancel_active(
    app: AppHandle,
    state: State<'_, VoiceState>,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        if let Some(active) = take_active_session(&state).await {
            shutdown_active_session(active, true, &state).await;
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
        use lattice_audio::CaptureProvider;
        use lattice_voice::{
            normalize_final_transcript, SessionContext, SpeechEventSender, SpeechProvider,
            SpeechSessionConfig, VoiceContextBuilder, VoiceContextInput, VoiceEvent,
        };

        // Preempt any leftover session from a release-during-start race.
        if let Some(stale) = take_active_session(&state).await {
            shutdown_active_session(stale, true, &state).await;
        }

        let provider = ensure_provider(&app, &state).await?;

        let session_id = format!("voice-{}", NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
        let (events, mut rx) = SpeechEventSender::pair();
        let hints = hints.unwrap_or_default();
        let context_input = VoiceContextInput {
            document_id: hints.document_id.clone(),
            heading_path: hints.heading_path.clone(),
            page_title: hints.page_title.clone(),
            workspace_name: hints.workspace_name.clone(),
            document_path: hints.document_path.clone(),
            tags: hints.tags.clone(),
            extra_glossary_terms: hints.glossary_terms.clone(),
            known_paths: hints.known_paths.clone(),
            known_symbols: Vec::new(),
        };
        let context = if hints.document_id.is_some()
            || hints.document_path.is_some()
            || hints.page_title.is_some()
            || hints.workspace_name.is_some()
            || !hints.tags.is_empty()
            || !hints.heading_path.is_empty()
            || !hints.glossary_terms.is_empty()
            || !hints.known_paths.is_empty()
        {
            VoiceContextBuilder::new().build_session_context(&context_input, false, None)
        } else {
            SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                known_paths: Vec::new(),
                command_mode: false,
            }
        };
        let normalization_context = lattice_voice::NormalizationContext::from(&context);
        let config = SpeechSessionConfig {
            session_id: session_id.clone(),
            language: Some("en".into()),
            context,
        };

        let speech_session = provider
            .start_session(config, events)
            .await
            .map_err(|err| err.to_string())?;

        let app_forward = app.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // Finals are emitted from `voice_finish_session` to avoid duplicates.
                let ui = match event {
                    VoiceEvent::PartialTranscript(payload) => VoiceUiEvent::Partial {
                        session_id: payload.session_id,
                        revision: payload.revision,
                        text: payload.text,
                    },
                    VoiceEvent::StableTranscript(payload) => VoiceUiEvent::Partial {
                        session_id: payload.session_id,
                        revision: payload.revision,
                        text: payload.text,
                    },
                    VoiceEvent::SessionFailed {
                        session_id,
                        message,
                        ..
                    } => VoiceUiEvent::Failed {
                        session_id: Some(session_id),
                        message,
                    },
                    VoiceEvent::ModelStatusChanged(status) => VoiceUiEvent::Status {
                        state: format!("{:?}", status.state).to_ascii_lowercase(),
                        message: status.message,
                    },
                    VoiceEvent::FinalTranscript(_)
                    | VoiceEvent::SessionReady { .. }
                    | VoiceEvent::SpeechStarted { .. }
                    | VoiceEvent::CommandCandidate(_)
                    | VoiceEvent::SessionCompleted { .. } => continue,
                };
                let _ = app_forward.emit(VOICE_EVENT, ui);
            }
        });

        let session_slot = Arc::new(Mutex::new(Some(speech_session)));

        // Start native capture (flushes armed pre-roll, then live frames).
        let capture_rx = {
            let mut inner = state.inner.lock().await;
            ensure_capture(&mut inner)?;
            let rx = inner
                .capture_rx
                .take()
                .ok_or_else(|| "native capture subscriber missing".to_string())?;
            let Some(capture) = inner.capture.as_mut() else {
                inner.capture_rx = Some(rx);
                return Err("native capture provider missing".into());
            };
            if let Err(err) = capture.start() {
                inner.capture_rx = Some(rx);
                return Err(err.to_string());
            }
            inner.capture_armed = false;
            rx
        };

        let pump = tokio::spawn(pump_capture_to_session(
            capture_rx,
            Arc::clone(&session_slot),
            session_id.clone(),
            app.clone(),
        ));

        // Another start may have raced; keep only this session.
        if let Some(stale) = take_active_session(&state).await {
            shutdown_active_session(stale, false, &state).await;
        }
        {
            let mut inner = state.inner.lock().await;
            inner.active = Some(ActiveSession {
                session_id: session_id.clone(),
                normalization_context,
                session: session_slot,
                _forwarder: forwarder,
                pump,
            });
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
        let _ = (app, state);
        Err(unsupported())
    }
}

/// Retired: WebView JSON float arrays are no longer accepted.
/// Native capture pushes PCM in-process from Rust.
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
        let mut inner = state.inner.lock().await;
        let active = inner
            .active
            .take()
            .ok_or_else(|| "no active dictation session".to_string())?;
        if active.session_id != session_id {
            inner.active = Some(active);
            return Err("session id mismatch".into());
        }

        // Stop capture before finishing so the pump drains and releases the session.
        stop_capture_and_rearm(&mut inner);
        drop(inner);

        let ActiveSession {
            session,
            _forwarder,
            pump,
            session_id: sid,
            normalization_context,
        } = active;
        // Stop forwarding immediately so late partials cannot re-paint provisional
        // ghost text after the authoritative final is inserted.
        _forwarder.abort();
        pump.abort();

        let mut speech = {
            let mut guard = session.lock().await;
            guard
                .take()
                .ok_or_else(|| "dictation session already closed".to_string())?
        };

        let _ = app.emit(
            VOICE_EVENT,
            VoiceUiEvent::Status {
                state: "finalizing".into(),
                message: Some("Finalizing transcript…".into()),
            },
        );

        match speech.finish_utterance().await {
            Ok(final_transcript) => {
                let final_transcript =
                    normalize_final_transcript(final_transcript, &normalization_context);
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Final {
                        session_id: sid,
                        text: final_transcript.text,
                        replaces_revision: Some(final_transcript.replaces_revision),
                        raw_text: final_transcript.raw_text,
                        corrections: final_transcript.corrections,
                    },
                );
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Status {
                        state: "idle".into(),
                        message: None,
                    },
                );
                Ok(())
            }
            Err(err) => {
                let message = err.to_string();
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Failed {
                        session_id: Some(sid),
                        message: message.clone(),
                    },
                );
                Err(message)
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
        let mut inner = state.inner.lock().await;
        let Some(active) = inner.active.take() else {
            return Ok(());
        };
        if active.session_id != session_id {
            inner.active = Some(active);
            return Err("session id mismatch".into());
        }
        stop_capture_and_rearm(&mut inner);
        drop(inner);

        active.pump.abort();
        active._forwarder.abort();
        {
            let mut guard = active.session.lock().await;
            if let Some(session) = guard.take() {
                let _ = session.cancel().await;
            }
        }
        let _ = app.emit(
            VOICE_EVENT,
            VoiceUiEvent::Status {
                state: "idle".into(),
                message: Some("Dictation cancelled".into()),
            },
        );
        Ok(())
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (app, state, session_id);
        Err(unsupported())
    }
}

#[cfg(all(test, target_os = "macos", feature = "voice"))]
mod tests {
    use super::chunk_from_frame;
    use lattice_audio::AudioFrame;

    #[test]
    fn chunk_from_frame_preserves_sequence_and_timestamp() {
        let frame = AudioFrame::from_f32_le(7, 1_234_567_890, &[0.25, -0.5], false);
        let chunk = chunk_from_frame("voice-1", &frame);
        assert_eq!(chunk.session_id, "voice-1");
        assert_eq!(chunk.sequence, 7);
        assert_eq!(chunk.captured_at_ns, 1_234_567_890);
        assert_ne!(chunk.captured_at_ns, 0);
        assert_eq!(chunk.sample_rate_hz, 16_000);
        assert_eq!(chunk.channels, 1);
        assert_eq!(chunk.payload.len(), 8);
    }
}
