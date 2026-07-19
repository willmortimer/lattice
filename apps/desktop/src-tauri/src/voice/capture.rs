//! Native microphone capture helpers shared by daemon and embedded backends.

use tauri::{AppHandle, Emitter};

use super::{VoiceInner, VoiceUiEvent, VOICE_EVENT};

pub(super) fn ensure_capture(inner: &mut VoiceInner) -> Result<(), String> {
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
        let capture = inner.capture.as_mut().expect("capture created above");
        capture.arm().map_err(|err| err.to_string())?;
        inner.capture_armed = true;
    }

    Ok(())
}

pub(super) fn stop_capture_and_rearm(inner: &mut VoiceInner) {
    use lattice_audio::CaptureProvider;

    let Some(capture) = inner.capture.as_mut() else {
        return;
    };
    let _ = capture.stop();
    inner.capture_armed = false;
    let rx = capture.subscribe();
    inner.capture_rx = Some(rx);
    match capture.arm() {
        Ok(()) => inner.capture_armed = true,
        Err(err) => {
            eprintln!("voice: failed to re-arm capture after stop: {err}");
        }
    }
}

/// Pump native frames into an async sink (daemon PushAudioChunk or in-process session).
pub(super) async fn pump_capture_frames<F, Fut>(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<lattice_audio::CaptureEvent>,
    session_id: String,
    app: AppHandle,
    mut on_frame: F,
) where
    F: FnMut(lattice_audio::AudioFrame) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
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
                    if let Err(message) = on_frame(frame).await {
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
