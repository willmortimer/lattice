//! In-process voice dictation for the Tauri desktop shell (M2).
//!
//! The WebView owns microphone capture and pushes Float32 @ 16 kHz mono
//! samples here. Recognition runs through `FluidAudioSpeechProvider` when
//! built with `--features voice` on macOS arm64.

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
}

#[cfg(all(target_os = "macos", feature = "voice"))]
struct ActiveSession {
    session_id: String,
    sequence: u64,
    session: Box<dyn lattice_voice::SpeechSession>,
    _forwarder: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    pub available: bool,
    pub prepared: bool,
    pub preparing: bool,
    pub listening: bool,
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
    "voice dictation requires macOS arm64 with `--features voice` and the FluidAudio bridge"
        .into()
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
async fn cancel_taken_session(active: ActiveSession) {
    let ActiveSession { session, .. } = active;
    let _ = session.cancel().await;
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
            cancel_taken_session(active).await;
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
        use lattice_voice::{
            SessionContext, SpeechEventSender, SpeechProvider, SpeechSessionConfig, VoiceContextBuilder,
            VoiceContextInput, VoiceEvent,
        };

        // Preempt any leftover session from a release-during-start race.
        if let Some(stale) = take_active_session(&state).await {
            cancel_taken_session(stale).await;
        }

        let provider = ensure_provider(&app, &state).await?;

        let session_id = format!("voice-{}", NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
        let (events, mut rx) = SpeechEventSender::pair();
        let hints = hints.unwrap_or_default();
        let context = if hints.document_id.is_some()
            || hints.document_path.is_some()
            || hints.page_title.is_some()
            || hints.workspace_name.is_some()
            || !hints.tags.is_empty()
            || !hints.heading_path.is_empty()
            || !hints.glossary_terms.is_empty()
        {
            VoiceContextBuilder::new().build_session_context(
                &VoiceContextInput {
                    document_id: hints.document_id,
                    heading_path: hints.heading_path,
                    page_title: hints.page_title,
                    workspace_name: hints.workspace_name,
                    document_path: hints.document_path,
                    tags: hints.tags,
                    extra_glossary_terms: hints.glossary_terms,
                    ..VoiceContextInput::default()
                },
                false,
                None,
            )
        } else {
            SessionContext {
                document_id: None,
                glossary_terms: Vec::new(),
                command_mode: false,
            }
        };
        let config = SpeechSessionConfig {
            session_id: session_id.clone(),
            language: Some("en".into()),
            context,
        };

        let session = provider
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

        // Another start may have raced; keep only this session.
        if let Some(stale) = take_active_session(&state).await {
            cancel_taken_session(stale).await;
        }
        {
            let mut inner = state.inner.lock().await;
            inner.active = Some(ActiveSession {
                session_id: session_id.clone(),
                sequence: 0,
                session,
                _forwarder: forwarder,
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

#[tauri::command]
pub async fn voice_push_audio(
    state: State<'_, VoiceState>,
    session_id: String,
    samples: Vec<f32>,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", feature = "voice"))]
    {
        use bytes::Bytes;
        use lattice_voice::{AudioChunk, AudioSampleFormat};

        let mut inner = state.inner.lock().await;
        let active = inner
            .active
            .as_mut()
            .ok_or_else(|| "no active dictation session".to_string())?;
        if active.session_id != session_id {
            return Err("session id mismatch".into());
        }
        if samples.is_empty() {
            return Ok(());
        }

        let mut payload = Vec::with_capacity(samples.len() * 4);
        for sample in &samples {
            payload.extend_from_slice(&sample.to_le_bytes());
        }

        let sequence = active.sequence;
        active.sequence += 1;
        let chunk = AudioChunk {
            session_id: session_id.clone(),
            sequence,
            captured_at_ns: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            sample_format: AudioSampleFormat::F32,
            payload: Bytes::from(payload),
        };
        active
            .session
            .push_audio(chunk)
            .await
            .map_err(|err| err.to_string())
    }
    #[cfg(not(all(target_os = "macos", feature = "voice")))]
    {
        let _ = (state, session_id, samples);
        Err(unsupported())
    }
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

        let _ = app.emit(
            VOICE_EVENT,
            VoiceUiEvent::Status {
                state: "finalizing".into(),
                message: Some("Finalizing transcript…".into()),
            },
        );

        let ActiveSession {
            mut session,
            _forwarder,
            session_id: sid,
            ..
        } = active;
        // Stop forwarding immediately so late partials cannot re-paint provisional
        // ghost text after the authoritative final is inserted.
        _forwarder.abort();
        // Drop the lock while finishing.
        drop(inner);

        match session.finish_utterance().await {
            Ok(final_transcript) => {
                let _ = app.emit(
                    VOICE_EVENT,
                    VoiceUiEvent::Final {
                        session_id: sid,
                        text: final_transcript.text,
                        replaces_revision: Some(final_transcript.replaces_revision),
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
        let ActiveSession { session, _forwarder, .. } = active;
        _forwarder.abort();
        drop(inner);
        let _ = session.cancel().await;
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
