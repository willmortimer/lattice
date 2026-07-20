//! Daemon thin-client path: `DaemonClient` ↔ latticed ↔ lattice-voice-host.
//!
//! Native capture stays in the Tauri process; packed PCM is streamed via
//! `PushAudioChunk`. Model ownership lives in latticed (ADR 0043).
//!
//! Connect/spawn is shared via [`crate::daemon_session`]. Voice may auto-enable
//! `LATTICE_VOICE_FAKE` when discovering voice-host; the first spawner owns the
//! child and sets `LATTICE_AUTH_TOKEN` so semantic can attach.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lattice_client::{
    request, response, DaemonClient, EventFilter, LatticeClient, Request,
};
use lattice_protocol::{
    event, AudioSampleFormat, CancelVoiceSessionRequest, EndVoiceSessionRequest,
    FinishUtteranceRequest, PrepareModelRequest, PushAudioChunkRequest, SessionContext,
    SpeechSessionConfig, StartVoiceSessionRequest,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};

use crate::daemon_session::{self, SpawnHostEnv, SpawnedDaemon};

use super::capture::{ensure_capture, pump_capture_frames, stop_capture_and_rearm};
use super::{
    VoiceInner, VoiceSessionContextHints, VoiceUiEvent, VOICE_EVENT,
};

/// Force daemon-only voice (no in-process FluidAudio fallback).
pub const ENV_VOICE_DAEMON: &str = "LATTICE_VOICE_DAEMON";
pub use crate::daemon_session::{ENV_AUTH_TOKEN, ENV_SOCKET};

const PREPARE_MODEL_ID: &str = "parakeet-unified-320ms";
const FAKE_PREPARE_MODEL_ID: &str = "null-0.1";

pub(super) struct DaemonBackend {
    pub client: Arc<DaemonClient>,
    /// Keeps a desktop-spawned daemon alive for the app lifetime.
    /// First spawner (voice or semantic) owns the child; the other attaches via socket.
    pub _child: Option<SpawnedDaemon>,
    pub prepared: bool,
    /// Whether the connected daemon is expected to run a fake voice-host.
    pub fake_host: bool,
}

pub(super) struct DaemonActiveSession {
    pub session_id: String,
    pub client: Arc<DaemonClient>,
    pub pump: tokio::task::JoinHandle<()>,
    pub forwarder: tokio::task::JoinHandle<()>,
    pub final_rx: oneshot::Receiver<DaemonFinal>,
    /// Glossary / paths for deterministic ITN before editor commit (parity with embedded).
    pub normalization_context: lattice_voice::NormalizationContext,
}

pub(super) struct DaemonFinal {
    pub text: String,
    pub replaces_revision: u64,
}

pub(super) fn daemon_required() -> bool {
    env_truthy(ENV_VOICE_DAEMON)
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn resolve_voice_host_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("LATTICE_VOICE_HOST_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(path) = daemon_session::which_bin("lattice-voice-host") {
        return Some(path);
    }
    if let Some(path) = daemon_session::current_exe_sibling("lattice-voice-host") {
        return Some(path);
    }
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/lattice-voice-host"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/lattice-voice-host"),
        PathBuf::from("target/debug/lattice-voice-host"),
        PathBuf::from("target/release/lattice-voice-host"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn prepare_model_id(fake_host: bool) -> &'static str {
    if fake_host || env_truthy("LATTICE_VOICE_FAKE") {
        FAKE_PREPARE_MODEL_ID
    } else {
        PREPARE_MODEL_ID
    }
}

/// Voice-host spawn env: may discover voice-host and auto-enable Fake.
///
/// Returns `(host_env, auto_fake)` where `auto_fake` is true when this call
/// will inject `LATTICE_VOICE_FAKE=1` for a discovered host binary.
fn voice_spawn_host_env() -> (SpawnHostEnv, bool) {
    let mut extra_env = Vec::new();
    let mut auto_fake = false;

    // Wire voice-host supervision when the parent did not already configure it.
    // Prefer an explicit fluidaudio host (`LATTICE_VOICE_HOST_BIN` without FAKE);
    // otherwise auto-enable the offline fake backend so thin-client smoke works.
    let host_bin_set = std::env::var_os("LATTICE_VOICE_HOST_BIN")
        .filter(|v| !v.is_empty())
        .is_some();
    let host_socket_set = std::env::var_os("LATTICE_VOICE_HOST_SOCKET")
        .filter(|v| !v.is_empty())
        .is_some();
    if !host_bin_set && !host_socket_set {
        if let Some(host) = resolve_voice_host_bin() {
            extra_env.push((
                "LATTICE_VOICE_HOST_BIN".to_string(),
                host.to_string_lossy().into(),
            ));
            // Safe default for auto-discovered host binaries (often built without
            // `--features fluidaudio`). Leave LATTICE_VOICE_FAKE alone when the
            // parent already set it (including empty / `0` for fluidaudio).
            if std::env::var_os("LATTICE_VOICE_FAKE").is_none() {
                extra_env.push(("LATTICE_VOICE_FAKE".to_string(), "1".into()));
                auto_fake = true;
            }
        }
    }

    (
        SpawnHostEnv {
            extra_env,
            handshake_hint: Some(
                "ensure voice-host is available: build lattice-voice-host, or set \
                 LATTICE_VOICE_FAKE=1 LATTICE_VOICE_HOST_BIN=…",
            ),
        },
        auto_fake,
    )
}

/// Connect to an existing latticed, or spawn one with voice-host env.
///
/// The third tuple element is whether the daemon is expected to run a fake
/// voice-host (`LATTICE_VOICE_FAKE` already set, or auto-enabled on spawn).
pub(super) async fn connect_or_spawn()
-> Result<(Arc<DaemonClient>, Option<SpawnedDaemon>, bool), String> {
    let ambient_fake = env_truthy("LATTICE_VOICE_FAKE");
    let (host_env, auto_fake) = voice_spawn_host_env();
    // auto_fake only applies when we actually spawn; connecting to an existing
    // socket ignores spawn env. Detect spawn vs connect via child Option.
    let (client, child) = daemon_session::connect_or_spawn(host_env).await?;
    let fake_host = ambient_fake || (child.is_some() && auto_fake);
    Ok((client, child, fake_host))
}

pub(super) async fn prepare(
    app: &AppHandle,
    backend: &mut DaemonBackend,
) -> Result<(), String> {
    if backend.prepared {
        return Ok(());
    }

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Status {
            state: "preparing".into(),
            message: Some("Preparing voice model via latticed…".into()),
        },
    );

    let model_id = prepare_model_id(backend.fake_host).to_string();
    let prepared = backend
        .client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::PrepareModel(PrepareModelRequest {
                model_id,
                warm: true,
            })),
        })
        .await
        .map_err(|err| {
            format!(
                "latticed PrepareModel failed: {err} \
                 (start latticed with a voice-host: LATTICE_VOICE_FAKE=1 \
                 LATTICE_VOICE_HOST_BIN=…, or a fluidaudio-featured host; \
                 or point {ENV_SOCKET}/{ENV_AUTH_TOKEN} at a voice-capable daemon)"
            )
        })?;

    match prepared.body {
        Some(response::Body::PrepareModel(_)) => {
            backend.prepared = true;
            let _ = app.emit(
                VOICE_EVENT,
                VoiceUiEvent::Status {
                    state: "ready".into(),
                    message: Some("Voice ready via latticed".into()),
                },
            );
            Ok(())
        }
        other => Err(format!("unexpected PrepareModel response: {other:?}")),
    }
}

pub(super) fn push_chunk_from_frame(
    session_id: &str,
    frame: &lattice_audio::AudioFrame,
) -> PushAudioChunkRequest {
    PushAudioChunkRequest {
        session_id: session_id.to_string(),
        sequence: frame.sequence,
        captured_at_ns: frame.captured_at_ns,
        sample_rate_hz: frame.format.sample_rate_hz,
        channels: u32::from(frame.format.channels),
        sample_format: AudioSampleFormat::F32 as i32,
        payload: frame.payload.to_vec(),
    }
}

fn session_context_from_hints(hints: &VoiceSessionContextHints) -> SessionContext {
    use lattice_voice::{VoiceContextBuilder, VoiceContextInput};

    let has_signals = hints.document_id.is_some()
        || hints.document_path.is_some()
        || hints.page_title.is_some()
        || hints.workspace_name.is_some()
        || !hints.tags.is_empty()
        || !hints.heading_path.is_empty()
        || !hints.glossary_terms.is_empty()
        || !hints.known_paths.is_empty();
    if !has_signals {
        return SessionContext {
            document_id: None,
            glossary_terms: Vec::new(),
            known_paths: Vec::new(),
            command_mode: false,
        };
    }
    let input = VoiceContextInput {
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
    let built = VoiceContextBuilder::new().build_session_context(&input, false, None);
    SessionContext {
        document_id: built.document_id,
        glossary_terms: built.glossary_terms,
        known_paths: built.known_paths,
        command_mode: built.command_mode,
    }
}

pub(super) async fn start_session(
    app: AppHandle,
    inner: &mut VoiceInner,
    session_id: String,
    hints: VoiceSessionContextHints,
) -> Result<DaemonActiveSession, String> {
    let backend = inner
        .daemon
        .as_ref()
        .ok_or_else(|| "daemon voice backend not prepared".to_string())?;
    let client = Arc::clone(&backend.client);

    let context = session_context_from_hints(&hints);
    let normalization_context = lattice_voice::NormalizationContext {
        glossary_terms: context.glossary_terms.clone(),
        known_paths: context.known_paths.clone(),
    };
    let started = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::StartVoiceSession(StartVoiceSessionRequest {
                config: Some(SpeechSessionConfig {
                    session_id: session_id.clone(),
                    language: Some("en".into()),
                    context: Some(context),
                    endpoint: None,
                }),
            })),
        })
        .await
        .map_err(|err| format!("StartVoiceSession failed: {err}"))?;

    match started.body {
        Some(response::Body::StartVoiceSession(resp)) => {
            if resp.session_id != session_id {
                return Err(format!(
                    "session id mismatch from daemon: got {}",
                    resp.session_id
                ));
            }
        }
        other => return Err(format!("unexpected StartVoiceSession response: {other:?}")),
    }

    let mut events = client
        .subscribe(EventFilter {
            workspace_id: None,
        })
        .await
        .map_err(|err| format!("subscribe voice events failed: {err}"))?;

    let (final_tx, final_rx) = oneshot::channel();
    let final_tx = Arc::new(Mutex::new(Some(final_tx)));
    let app_forward = app.clone();
    let forward_session = session_id.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(item) = events.next().await {
            let Ok(event) = item else {
                break;
            };
            let Some(body) = event.body else {
                continue;
            };
            let ui = match body {
                event::Body::PartialTranscript(payload) => {
                    if payload.session_id != forward_session {
                        continue;
                    }
                    VoiceUiEvent::Partial {
                        session_id: payload.session_id,
                        revision: payload.revision,
                        text: payload.text,
                    }
                }
                event::Body::StableTranscript(payload) => {
                    if payload.session_id != forward_session {
                        continue;
                    }
                    VoiceUiEvent::Partial {
                        session_id: payload.session_id,
                        revision: payload.revision,
                        text: payload.text,
                    }
                }
                event::Body::FinalTranscript(payload) => {
                    if payload.session_id != forward_session {
                        continue;
                    }
                    // Deliver to finish_session (avoid duplicate UI finals).
                    if let Some(tx) = final_tx.lock().await.take() {
                        let _ = tx.send(DaemonFinal {
                            text: payload.text,
                            replaces_revision: payload.replaces_revision,
                        });
                    }
                    continue;
                }
                event::Body::SessionFailed(failed) => {
                    if !failed.session_id.is_empty() && failed.session_id != forward_session {
                        continue;
                    }
                    VoiceUiEvent::Failed {
                        session_id: Some(failed.session_id),
                        message: failed.message,
                    }
                }
                event::Body::ModelStatus(changed) => {
                    let status = changed.status.unwrap_or_default();
                    VoiceUiEvent::Status {
                        state: format!("model_{}", status.state),
                        message: status.message,
                    }
                }
                event::Body::AudioGap(gap) => VoiceUiEvent::Status {
                    state: "listening".into(),
                    message: Some(format!(
                        "audio gap: from {} to {}{}",
                        gap.last_contiguous_sequence,
                        gap.next_sequence,
                        gap.reason
                            .as_ref()
                            .map(|r| format!(" ({r})"))
                            .unwrap_or_default()
                    )),
                },
                event::Body::SessionReady(_)
                | event::Body::SpeechStarted(_)
                | event::Body::EndpointDetected(_)
                | event::Body::SessionCompleted(_)
                | event::Body::CommandCandidate(_) => continue,
                _ => continue,
            };
            let _ = app_forward.emit(VOICE_EVENT, ui);
        }
    });

    ensure_capture(inner)?;
    let capture_rx = {
        let rx = inner
            .capture_rx
            .take()
            .ok_or_else(|| "native capture subscriber missing".to_string())?;
        use lattice_audio::CaptureProvider;
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

    let pump_client = Arc::clone(&client);
    let pump_session = session_id.clone();
    let pump_app = app.clone();
    let pump = tokio::spawn(async move {
        pump_capture_frames(capture_rx, pump_session.clone(), pump_app, move |frame| {
            let client = Arc::clone(&pump_client);
            let session_id = pump_session.clone();
            async move {
                let chunk = push_chunk_from_frame(&session_id, &frame);
                client
                    .request(Request {
                        deadline_unix_ms: None,
                        idempotency_key: None,
                        body: Some(request::Body::PushAudioChunk(chunk)),
                    })
                    .await
                    .map_err(|err| err.to_string())?;
                Ok(())
            }
        })
        .await;
    });

    Ok(DaemonActiveSession {
        session_id,
        client,
        pump,
        forwarder,
        final_rx,
        normalization_context,
    })
}

pub(super) async fn finish_session(
    app: &AppHandle,
    active: DaemonActiveSession,
    inner: &mut VoiceInner,
) -> Result<(), String> {
    use lattice_voice::{
        normalize_final_transcript, FinalTranscript, FinalizationMode,
    };

    stop_capture_and_rearm(inner);
    active.pump.abort();
    // Keep forwarder alive until final arrives, then abort.

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Status {
            state: "finalizing".into(),
            message: Some("Finalizing transcript…".into()),
        },
    );

    active
        .client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::FinishUtterance(FinishUtteranceRequest {
                session_id: active.session_id.clone(),
                utterance_id: format!("{}-utt", active.session_id),
            })),
        })
        .await
        .map_err(|err| format!("FinishUtterance failed: {err}"))?;

    let daemon_final = tokio::time::timeout(Duration::from_secs(30), active.final_rx)
        .await
        .map_err(|_| "timed out waiting for FinalTranscript from latticed".to_string())?
        .map_err(|_| "final transcript channel closed before FinalTranscript".to_string())?;

    active.forwarder.abort();

    // Production stays on StreamingFlush (voice-eval deferred IndependentOfflineRedecode).
    let final_transcript = normalize_final_transcript(
        FinalTranscript {
            session_id: active.session_id.clone(),
            utterance_id: format!("{}-utt", active.session_id),
            replaces_revision: daemon_final.replaces_revision,
            text: daemon_final.text,
            raw_text: None,
            corrections: Vec::new(),
            finalization_mode: FinalizationMode::StreamingFlush,
            duration_ms: 0,
            processing_ms: 0,
        },
        &active.normalization_context,
    );

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Final {
            session_id: active.session_id.clone(),
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

    let _ = active
        .client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::EndVoiceSession(EndVoiceSessionRequest {
                session_id: active.session_id,
            })),
        })
        .await;

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Status {
            state: "idle".into(),
            message: None,
        },
    );
    Ok(())
}

pub(super) async fn cancel_session(
    app: &AppHandle,
    active: DaemonActiveSession,
    inner: &mut VoiceInner,
) -> Result<(), String> {
    stop_capture_and_rearm(inner);
    active.pump.abort();
    active.forwarder.abort();
    let _ = active
        .client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::CancelVoiceSession(
                CancelVoiceSessionRequest {
                    session_id: active.session_id,
                    reason: Some("client cancel".into()),
                },
            )),
        })
        .await;
    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Status {
            state: "idle".into(),
            message: Some("Dictation cancelled".into()),
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_audio::AudioFrame;

    #[test]
    fn push_chunk_preserves_sequence_and_timestamp() {
        let frame = AudioFrame::from_f32_le(7, 1_234_567_890, &[0.25, -0.5], false);
        let chunk = push_chunk_from_frame("voice-1", &frame);
        assert_eq!(chunk.session_id, "voice-1");
        assert_eq!(chunk.sequence, 7);
        assert_eq!(chunk.captured_at_ns, 1_234_567_890);
        assert_eq!(chunk.sample_rate_hz, 16_000);
        assert_eq!(chunk.channels, 1);
        assert_eq!(chunk.sample_format, AudioSampleFormat::F32 as i32);
        assert_eq!(chunk.payload.len(), 8);
    }

    #[test]
    fn session_context_merges_glossary_hints() {
        let hints = VoiceSessionContextHints {
            document_id: Some("doc".into()),
            document_path: Some("notes/a.md".into()),
            page_title: Some("Title".into()),
            workspace_name: None,
            tags: vec!["tag".into()],
            heading_path: vec!["H1".into()],
            glossary_terms: vec!["Lattice".into()],
            known_paths: vec!["crates/foo".into()],
        };
        let ctx = session_context_from_hints(&hints);
        assert_eq!(ctx.document_id.as_deref(), Some("doc"));
        assert!(ctx.glossary_terms.iter().any(|t| t == "Lattice"));
        assert!(ctx.glossary_terms.iter().any(|t| t == "tag"));
        assert!(ctx.glossary_terms.iter().any(|t| t == "Title"));
        assert!(ctx.glossary_terms.iter().all(|t| t != "crates/foo"));
        assert_eq!(ctx.known_paths, vec!["crates/foo".to_string()]);
    }

    #[test]
    fn session_context_feeds_normalization_context() {
        let hints = VoiceSessionContextHints {
            document_id: None,
            document_path: Some("Inbox/Sample capture.md".into()),
            page_title: Some("Sample capture".into()),
            workspace_name: Some("First Look".into()),
            tags: vec!["inbox".into()],
            heading_path: Vec::new(),
            glossary_terms: vec!["VoiceContextBuilder".into()],
            known_paths: vec!["Inbox/Sample capture".into()],
        };
        let ctx = session_context_from_hints(&hints);
        let norm = lattice_voice::NormalizationContext {
            glossary_terms: ctx.glossary_terms.clone(),
            known_paths: ctx.known_paths.clone(),
        };
        assert!(norm
            .glossary_terms
            .iter()
            .any(|t| t == "VoiceContextBuilder"));
        assert_eq!(norm.known_paths, vec!["Inbox/Sample capture".to_string()]);
    }

    #[test]
    fn daemon_required_reads_env_shape() {
        // Do not mutate process env in parallel tests; just exercise the helper API.
        let _ = daemon_required();
        let path = crate::daemon_session::socket_path();
        assert!(path.file_name().is_some());
    }
}
