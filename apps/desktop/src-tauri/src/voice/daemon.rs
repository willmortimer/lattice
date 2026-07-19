//! Daemon thin-client path: `DaemonClient` ↔ latticed ↔ lattice-voice-host.
//!
//! Native capture stays in the Tauri process; packed PCM is streamed via
//! `PushAudioChunk`. Model ownership lives in latticed (ADR 0043).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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

use super::capture::{ensure_capture, pump_capture_frames, stop_capture_and_rearm};
use super::{
    VoiceInner, VoiceSessionContextHints, VoiceUiEvent, VOICE_EVENT,
};

/// Force daemon-only voice (no in-process FluidAudio fallback).
pub const ENV_VOICE_DAEMON: &str = "LATTICE_VOICE_DAEMON";
/// Unix socket path for an existing or to-be-spawned `latticed`.
pub const ENV_SOCKET: &str = "LATTICE_SOCKET";
/// Handshake / API auth token (`latticed --auth-token` / `LATTICE_AUTH_TOKEN`).
pub const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
/// Optional path to the `latticed` binary for on-demand spawn.
pub const ENV_LATTICED_BIN: &str = "LATTICE_LATTICED_BIN";

const PREPARE_MODEL_ID: &str = "parakeet-unified-320ms";
const FAKE_PREPARE_MODEL_ID: &str = "null-0.1";

pub(super) struct DaemonBackend {
    pub client: Arc<DaemonClient>,
    /// Keeps a desktop-spawned daemon alive for the app lifetime.
    pub _child: Option<SpawnedDaemon>,
    pub prepared: bool,
}

pub(super) struct DaemonActiveSession {
    pub session_id: String,
    pub client: Arc<DaemonClient>,
    pub pump: tokio::task::JoinHandle<()>,
    pub forwarder: tokio::task::JoinHandle<()>,
    pub final_rx: oneshot::Receiver<DaemonFinal>,
}

pub(super) struct DaemonFinal {
    pub text: String,
    pub replaces_revision: u64,
}

pub(super) struct SpawnedDaemon {
    child: Child,
}

impl Drop for SpawnedDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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

fn default_socket_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join("run")
        .join("latticed.sock")
}

pub(super) fn socket_path() -> PathBuf {
    std::env::var_os(ENV_SOCKET)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_socket_path)
}

fn resolve_latticed_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_LATTICED_BIN) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(path) = which_bin("latticed") {
        return Some(path);
    }
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/latticed"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/latticed"),
        PathBuf::from("target/debug/latticed"),
        PathBuf::from("target/release/latticed"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn which_bin(name: &str) -> std::io::Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "PATH not set")
    })?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{name} not found on PATH"),
    ))
}

fn prepare_model_id() -> &'static str {
    if env_truthy("LATTICE_VOICE_FAKE") {
        FAKE_PREPARE_MODEL_ID
    } else {
        PREPARE_MODEL_ID
    }
}

/// Connect to an existing latticed, or spawn one (mirrors `lattice_daemon::spawn_latticed`).
pub(super) async fn connect_or_spawn() -> Result<(Arc<DaemonClient>, Option<SpawnedDaemon>), String>
{
    let socket = socket_path();
    let env_token = std::env::var(ENV_AUTH_TOKEN).ok().filter(|t| !t.is_empty());

    if socket.exists() {
        let token = env_token.ok_or_else(|| {
            format!(
                "latticed socket exists at {} but {ENV_AUTH_TOKEN} is unset; \
                 pass the daemon auth token or unset the stale socket",
                socket.display()
            )
        })?;
        let client = DaemonClient::connect(&socket, &token)
            .await
            .map_err(|err| format!("connect to latticed at {}: {err}", socket.display()))?;
        return Ok((Arc::new(client), None));
    }

    let binary = resolve_latticed_bin().ok_or_else(|| {
        format!(
            "latticed not running at {} and no binary found \
             (set {ENV_LATTICED_BIN} or build `latticed`)",
            socket.display()
        )
    })?;
    let token = env_token.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let child = spawn_latticed(&binary, &socket, &token)?;
    wait_for_socket(&socket, Duration::from_secs(8))?;
    let client = DaemonClient::connect(&socket, &token)
        .await
        .map_err(|err| {
            format!(
                "spawned latticed at {} but handshake failed: {err} \
                 (ensure voice-host env is set: LATTICE_VOICE_FAKE=1 \
                 LATTICE_VOICE_HOST_BIN=…)",
                socket.display()
            )
        })?;
    Ok((Arc::new(client), Some(SpawnedDaemon { child })))
}

fn spawn_latticed(binary: &Path, socket: &Path, auth_token: &str) -> Result<Child, String> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if socket.exists() {
        let _ = std::fs::remove_file(socket);
    }
    // Mirrors apps/daemon/src/spawn.rs: private UDS, no HTTP API, keep-running for voice.
    Command::new(binary)
        .arg("--socket")
        .arg(socket)
        .arg("--auth-token")
        .arg(auth_token)
        .arg("--api-port")
        .arg("0")
        .arg("--keep-services-running")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to spawn {}: {err}", binary.display()))
}

fn wait_for_socket(socket: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if socket.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "timed out waiting for latticed socket {}",
        socket.display()
    ))
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

    let model_id = prepare_model_id().to_string();
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
                 (start latticed with LATTICE_VOICE_FAKE=1 and LATTICE_VOICE_HOST_BIN, \
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
    let mut glossary_terms = hints.glossary_terms.clone();
    for term in hints
        .tags
        .iter()
        .chain(hints.heading_path.iter())
        .chain(hints.known_paths.iter())
    {
        if !term.is_empty() && !glossary_terms.iter().any(|t| t == term) {
            glossary_terms.push(term.clone());
        }
    }
    if let Some(title) = hints.page_title.as_ref().filter(|t| !t.is_empty()) {
        if !glossary_terms.iter().any(|t| t == title) {
            glossary_terms.push(title.clone());
        }
    }
    SessionContext {
        document_id: hints
            .document_id
            .clone()
            .or_else(|| hints.document_path.clone()),
        glossary_terms,
        command_mode: false,
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
    })
}

pub(super) async fn finish_session(
    app: &AppHandle,
    active: DaemonActiveSession,
    inner: &mut VoiceInner,
) -> Result<(), String> {
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

    let final_transcript = tokio::time::timeout(Duration::from_secs(30), active.final_rx)
        .await
        .map_err(|_| "timed out waiting for FinalTranscript from latticed".to_string())?
        .map_err(|_| "final transcript channel closed before FinalTranscript".to_string())?;

    active.forwarder.abort();

    let _ = app.emit(
        VOICE_EVENT,
        VoiceUiEvent::Final {
            session_id: active.session_id.clone(),
            text: final_transcript.text,
            replaces_revision: Some(final_transcript.replaces_revision),
            raw_text: None,
            corrections: Vec::new(),
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
        assert!(ctx.glossary_terms.iter().any(|t| t == "crates/foo"));
        assert!(ctx.glossary_terms.iter().any(|t| t == "Title"));
    }

    #[test]
    fn daemon_required_reads_env_shape() {
        // Do not mutate process env in parallel tests; just exercise the helper API.
        let _ = daemon_required();
        let path = socket_path();
        assert!(path.file_name().is_some());
    }
}
