//! Transitional in-process FluidAudio path (degraded when latticed is unavailable).

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use super::capture::{ensure_capture, pump_capture_frames, stop_capture_and_rearm};
use super::{
    VoiceInner, VoiceSessionContextHints, VoiceUiEvent, VOICE_EVENT,
};

pub(super) struct EmbeddedBackend {
    pub provider: Arc<lattice_voice_macos::FluidAudioSpeechProvider>,
}

pub(super) struct EmbeddedActiveSession {
    pub session_id: String,
    pub normalization_context: lattice_voice::NormalizationContext,
    pub session: Arc<Mutex<Option<Box<dyn lattice_voice::SpeechSession>>>>,
    pub forwarder: tokio::task::JoinHandle<()>,
    pub pump: tokio::task::JoinHandle<()>,
}

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

pub(super) async fn ensure_provider(
    app: &AppHandle,
    inner: &mut VoiceInner,
) -> Result<Arc<lattice_voice_macos::FluidAudioSpeechProvider>, String> {
    use lattice_voice::{PrepareModelRequest, SpeechProvider};
    use lattice_voice_macos::FluidAudioSpeechProvider;

    if let Some(backend) = &inner.embedded {
        return Ok(Arc::clone(&backend.provider));
    }
    if inner.preparing {
        return Err("voice model prepare is already in progress; wait for it to finish".into());
    }

    inner.preparing = true;
    let provider = match FluidAudioSpeechProvider::new() {
        Ok(provider) => Arc::new(provider),
        Err(err) => {
            inner.preparing = false;
            return Err(err.to_string());
        }
    };

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Status {
            state: "preparing".into(),
            message: Some(
                "Preparing local voice model (degraded embedded FluidAudio; \
                 first run may take several minutes)…"
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

    inner.preparing = false;

    match prepare_result {
        Ok(_) => {
            if let Err(err) = ensure_capture(inner) {
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
            inner.embedded = Some(EmbeddedBackend {
                provider: Arc::clone(&provider),
            });
            let _ = app.emit(
                VOICE_EVENT,
                VoiceUiEvent::Status {
                    state: "ready".into(),
                    message: Some(
                        "Local voice model ready (degraded embedded — latticed preferred)".into(),
                    ),
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

pub(super) async fn start_session(
    app: AppHandle,
    inner: &mut VoiceInner,
    session_id: String,
    hints: VoiceSessionContextHints,
) -> Result<EmbeddedActiveSession, String> {
    use lattice_audio::CaptureProvider;
    use lattice_voice::{
        SessionContext, SpeechEventSender, SpeechProvider, SpeechSessionConfig, VoiceContextBuilder,
        VoiceContextInput, VoiceEvent,
    };

    let provider = ensure_provider(&app, inner).await?;

    let (events, mut rx) = SpeechEventSender::pair();
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
        endpoint: lattice_voice::EndpointOptions::default(),
    };

    let speech_session = provider
        .start_session(config, events)
        .await
        .map_err(|err| err.to_string())?;

    let app_forward = app.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
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
                | VoiceEvent::EndpointDetected { .. }
                | VoiceEvent::CommandCandidate(_)
                | VoiceEvent::SessionCompleted { .. } => continue,
            };
            let _ = app_forward.emit(VOICE_EVENT, ui);
        }
    });

    let session_slot = Arc::new(Mutex::new(Some(speech_session)));

    ensure_capture(inner)?;
    let capture_rx = {
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

    let pump_session = Arc::clone(&session_slot);
    let pump_id = session_id.clone();
    let pump_app = app.clone();
    let pump = tokio::spawn(async move {
        pump_capture_frames(capture_rx, pump_id.clone(), pump_app, move |frame| {
            let session = Arc::clone(&pump_session);
            let session_id = pump_id.clone();
            async move {
                let chunk = chunk_from_frame(&session_id, &frame);
                let mut guard = session.lock().await;
                let Some(speech) = guard.as_mut() else {
                    return Ok(());
                };
                speech
                    .push_audio(chunk)
                    .await
                    .map_err(|err| err.to_string())
            }
        })
        .await;
    });

    Ok(EmbeddedActiveSession {
        session_id,
        normalization_context,
        session: session_slot,
        forwarder,
        pump,
    })
}

pub(super) async fn finish_session(
    app: &AppHandle,
    active: EmbeddedActiveSession,
    inner: &mut VoiceInner,
) -> Result<(), String> {
    use lattice_voice::normalize_final_transcript;

    stop_capture_and_rearm(inner);
    active.forwarder.abort();
    active.pump.abort();

    let mut speech = {
        let mut guard = active.session.lock().await;
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
                normalize_final_transcript(final_transcript, &active.normalization_context);
            let _ = app.emit(
                VOICE_EVENT,
                VoiceUiEvent::Final {
                    session_id: active.session_id,
                    text: final_transcript.text,
                    replaces_revision: Some(final_transcript.replaces_revision),
                    raw_text: final_transcript.raw_text,
                    corrections: final_transcript
                        .corrections
                        .iter()
                        .map(super::VoiceTranscriptCorrection::from)
                        .collect(),
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
                    session_id: Some(active.session_id),
                    message: message.clone(),
                },
            );
            Err(message)
        }
    }
}

pub(super) async fn cancel_session(
    app: &AppHandle,
    active: EmbeddedActiveSession,
    inner: &mut VoiceInner,
) -> Result<(), String> {
    stop_capture_and_rearm(inner);
    active.pump.abort();
    active.forwarder.abort();
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

pub(super) async fn shutdown_active(active: EmbeddedActiveSession, rearm: bool, inner: &mut VoiceInner) {
    active.pump.abort();
    {
        let mut guard = active.session.lock().await;
        if let Some(session) = guard.take() {
            let _ = session.cancel().await;
        }
    }
    if rearm {
        stop_capture_and_rearm(inner);
    }
}

#[cfg(test)]
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
