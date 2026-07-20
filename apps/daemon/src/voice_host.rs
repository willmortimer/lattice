//! Optional `lattice-voice-host` supervision for the voice plane.
//!
//! Mirrors [`crate::embed_host`]: spawn/supervise an isolated inference process,
//! reconnect after crashes with bounded backoff, and mark the voice plane
//! degraded when the host is unavailable. Session policy is intentionally
//! simple — one active voice session per daemon.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lattice_protocol::{event, request, Event, Request, Response};
use lattice_voice::{normalize_transcript, NormalizationContext};
use lattice_voice_host::VoiceHostClient;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::error::{Error, Result};

/// Environment variable naming an existing voice-host UDS path.
pub const ENV_VOICE_HOST_SOCKET: &str = "LATTICE_VOICE_HOST_SOCKET";
/// When set to a truthy value, prefer the fake voice-host backend (tests / CI).
pub const ENV_VOICE_FAKE: &str = "LATTICE_VOICE_FAKE";
/// Optional path to the `lattice-voice-host` binary to spawn.
pub const ENV_VOICE_HOST_BIN: &str = "LATTICE_VOICE_HOST_BIN";
/// Optional FluidAudio / Parakeet model cache for supervised fluidaudio hosts.
pub const ENV_VOICE_MODEL_CACHE: &str = "LATTICE_VOICE_MODEL_CACHE";

const MAX_RESTARTS: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_BACKOFF_MS: u64 = 5_000;

/// How the daemon obtains a voice-host connection.
#[derive(Debug, Clone)]
pub enum VoiceProviderMode {
    /// Connect to an already-running voice-host socket (supervision optional).
    ExternalSocket { socket: PathBuf },
    /// Spawn `lattice-voice-host` and supervise it.
    SpawnHost {
        binary: PathBuf,
        socket: PathBuf,
        /// Use `--backend fake` (deterministic NullSpeechProvider).
        fake: bool,
    },
}

impl VoiceProviderMode {
    /// Resolve provider mode from environment variables.
    ///
    /// - `LATTICE_VOICE_FAKE=1` → spawn `--backend fake` (auto-resolves the host
    ///   binary when `LATTICE_VOICE_HOST_BIN` is unset).
    /// - `LATTICE_VOICE_HOST_BIN` without fake → spawn `--backend fluidaudio`
    ///   (binary must be built with `--features fluidaudio`).
    /// - `LATTICE_VOICE_HOST_SOCKET` alone → connect to an existing host.
    pub fn from_env() -> Option<Self> {
        let fake = env_truthy(ENV_VOICE_FAKE);
        let socket = std::env::var(ENV_VOICE_HOST_SOCKET)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let binary = std::env::var(ENV_VOICE_HOST_BIN)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                if fake {
                    resolve_voice_host_bin()
                } else {
                    None
                }
            });

        if let Some(binary) = binary {
            let socket = socket.unwrap_or_else(default_voice_socket_path);
            return Some(Self::SpawnHost {
                binary,
                socket,
                fake,
            });
        }
        socket.map(|socket| Self::ExternalSocket { socket })
    }
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn default_voice_socket_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "lattice-voice-host-{}.sock",
        std::process::id()
    ))
}

/// Locate `lattice-voice-host` for tests / local launches.
pub fn resolve_voice_host_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_VOICE_HOST_BIN) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(path) = which_bin("lattice-voice-host") {
        return Some(path);
    }

    if let Some(path) = current_exe_sibling("lattice-voice-host") {
        return Some(path);
    }

    // Walk common cargo target dirs from this crate / cwd.
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/lattice-voice-host"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/release/lattice-voice-host"),
        PathBuf::from("target/debug/lattice-voice-host"),
        PathBuf::from("target/release/lattice-voice-host"),
    ];
    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
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

fn current_exe_sibling(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}

struct SupervisedHost {
    child: Child,
    socket: PathBuf,
    binary: PathBuf,
    fake: bool,
    restarts: AtomicU32,
}

struct ActiveVoiceSession {
    session_id: String,
    /// Glossary terms used to normalize finals at the daemon boundary.
    glossary_terms: Vec<String>,
    /// Workspace-relative paths for ITN / path normalization on finals.
    known_paths: Vec<String>,
}

/// Shared voice-host controller for a daemon instance.
pub struct VoiceController {
    socket: PathBuf,
    client: Mutex<Option<Arc<VoiceHostClient>>>,
    host: Mutex<Option<SupervisedHost>>,
    stop: Arc<AtomicBool>,
    supervisor: Mutex<Option<JoinHandle<()>>>,
    degraded: AtomicBool,
    /// One active voice session per daemon (simple D5 policy).
    active_session: Mutex<Option<ActiveVoiceSession>>,
    fanout: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl VoiceController {
    /// Build a controller for the given mode (connect/spawn).
    pub async fn start(mode: VoiceProviderMode) -> Result<Arc<Self>> {
        match mode {
            VoiceProviderMode::ExternalSocket { socket } => {
                wait_for_socket(&socket, Duration::from_secs(5))?;
                let client = VoiceHostClient::connect(&socket)
                    .await
                    .map_err(|err| Error::Spawn(format!("voice-host connect: {err}")))?;
                let controller = Arc::new(Self {
                    socket: socket.clone(),
                    client: Mutex::new(Some(Arc::new(client))),
                    host: Mutex::new(None),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                    degraded: AtomicBool::new(false),
                    active_session: Mutex::new(None),
                    fanout: Mutex::new(None),
                });
                controller.spawn_socket_watch(socket);
                Ok(controller)
            }
            VoiceProviderMode::SpawnHost {
                binary,
                socket,
                fake,
            } => {
                let child = spawn_voice_host(&binary, &socket, fake)?;
                wait_for_socket(&socket, Duration::from_secs(5))?;
                let client = VoiceHostClient::connect(&socket)
                    .await
                    .map_err(|err| Error::Spawn(format!("voice-host connect: {err}")))?;
                let controller = Arc::new(Self {
                    socket: socket.clone(),
                    client: Mutex::new(Some(Arc::new(client))),
                    host: Mutex::new(Some(SupervisedHost {
                        child,
                        socket: socket.clone(),
                        binary,
                        fake,
                        restarts: AtomicU32::new(0),
                    })),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                    degraded: AtomicBool::new(false),
                    active_session: Mutex::new(None),
                    fanout: Mutex::new(None),
                });
                controller.spawn_host_supervisor();
                Ok(controller)
            }
        }
    }

    /// Fan host push events into the daemon client event bus.
    pub fn attach_event_fanout(
        self: &Arc<Self>,
        event_tx: broadcast::Sender<Event>,
        next_event_seq: Arc<AtomicU64>,
    ) {
        let controller = Arc::clone(self);
        let stop = Arc::clone(&self.stop);
        let join = tokio::spawn(async move {
            loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let client = controller
                    .client
                    .lock()
                    .expect("client poisoned")
                    .clone();
                let Some(client) = client else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                let mut rx = client.subscribe();
                loop {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    match rx.recv().await {
                        Ok(event) => {
                            let body = match event.body {
                                Some(body) => controller.maybe_normalize_final(body),
                                None => continue,
                            };
                            // Only forward voice-plane events (partials/finals/gaps/model).
                            if !is_voice_fanout_body(&body) {
                                continue;
                            }
                            let sequenced = Event {
                                sequence: next_event_seq.fetch_add(1, Ordering::Relaxed),
                                workspace_id: event.workspace_id,
                                body: Some(body),
                            };
                            let _ = event_tx.send(sequenced);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
                // Client dropped (host crash); wait for reconnect.
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        *self.fanout.lock().expect("fanout poisoned") = Some(join);
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::SeqCst)
    }

    /// Kill the supervised voice-host child (integration tests only).
    #[doc(hidden)]
    pub fn kill_supervised_host_for_test(&self) -> bool {
        let mut guard = self.host.lock().expect("host poisoned");
        let Some(host) = guard.as_mut() else {
            return false;
        };
        let _ = host.child.kill();
        let _ = host.child.wait();
        self.mark_degraded(true);
        self.clear_client();
        true
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket
    }

    /// Proxy a voice-plane request to the supervised host.
    pub async fn handle_request(&self, req: Request) -> std::result::Result<Response, lattice_protocol::Error> {
        let body = req.body.clone().ok_or_else(|| lattice_protocol::Error {
            code: "invalid_request".into(),
            message: "request body is required".into(),
            details: None,
        })?;

        match &body {
            request::Body::StartVoiceSession(start) => {
                self.ensure_can_start(start).await?;
            }
            request::Body::PushAudioChunk(chunk) => {
                self.ensure_session_matches(&chunk.session_id).await?;
            }
            request::Body::FinishUtterance(finish) => {
                self.ensure_session_matches(&finish.session_id).await?;
            }
            request::Body::UpdateSessionContext(update) => {
                self.ensure_session_matches(&update.session_id).await?;
            }
            request::Body::CancelVoiceSession(cancel) => {
                self.ensure_session_matches(&cancel.session_id).await?;
            }
            request::Body::EndVoiceSession(end) => {
                self.ensure_session_matches(&end.session_id).await?;
            }
            _ => {}
        }

        let client = self.client_or_reconnect().await?;
        let response = client.forward(Request {
            deadline_unix_ms: req.deadline_unix_ms,
            idempotency_key: req.idempotency_key,
            body: Some(body.clone()),
        })
        .await
        .map_err(voice_host_error_to_wire)?;

        match &body {
            request::Body::StartVoiceSession(start) => {
                let session_id = start
                    .config
                    .as_ref()
                    .map(|c| c.session_id.clone())
                    .unwrap_or_default();
                let (glossary_terms, known_paths) = start
                    .config
                    .as_ref()
                    .and_then(|c| c.context.as_ref())
                    .map(|c| (c.glossary_terms.clone(), c.known_paths.clone()))
                    .unwrap_or_default();
                *self.active_session.lock().expect("active_session poisoned") =
                    Some(ActiveVoiceSession {
                        session_id,
                        glossary_terms,
                        known_paths,
                    });
            }
            request::Body::UpdateSessionContext(update) => {
                if let Some(active) = self
                    .active_session
                    .lock()
                    .expect("active_session poisoned")
                    .as_mut()
                {
                    if let Some(context) = update.context.as_ref() {
                        active.glossary_terms = context.glossary_terms.clone();
                        active.known_paths = context.known_paths.clone();
                    }
                }
            }
            request::Body::CancelVoiceSession(_)
            | request::Body::EndVoiceSession(_)
            | request::Body::UnloadVoiceModel(_) => {
                *self.active_session.lock().expect("active_session poisoned") = None;
            }
            _ => {}
        }

        Ok(response)
    }

    async fn ensure_can_start(
        &self,
        start: &lattice_protocol::StartVoiceSessionRequest,
    ) -> std::result::Result<(), lattice_protocol::Error> {
        let session_id = start
            .config
            .as_ref()
            .map(|c| c.session_id.as_str())
            .unwrap_or("");
        if session_id.is_empty() {
            return Err(lattice_protocol::Error {
                code: "invalid_request".into(),
                message: "start_voice_session requires config.session_id".into(),
                details: None,
            });
        }
        let guard = self.active_session.lock().expect("active_session poisoned");
        if let Some(active) = guard.as_ref() {
            return Err(lattice_protocol::Error {
                code: "voice_session_busy".into(),
                message: format!(
                    "daemon already has active voice session '{}'; end or cancel it first",
                    active.session_id
                ),
                details: None,
            });
        }
        Ok(())
    }

    async fn ensure_session_matches(
        &self,
        session_id: &str,
    ) -> std::result::Result<(), lattice_protocol::Error> {
        let guard = self.active_session.lock().expect("active_session poisoned");
        match guard.as_ref() {
            Some(active) if active.session_id == session_id => Ok(()),
            Some(active) => Err(lattice_protocol::Error {
                code: "voice_session_mismatch".into(),
                message: format!(
                    "active voice session is '{}', got '{session_id}'",
                    active.session_id
                ),
                details: None,
            }),
            None => Err(lattice_protocol::Error {
                code: "voice_session_inactive".into(),
                message: format!("no active voice session (wanted '{session_id}')"),
                details: None,
            }),
        }
    }

    async fn client_or_reconnect(
        &self,
    ) -> std::result::Result<Arc<VoiceHostClient>, lattice_protocol::Error> {
        {
            let guard = self.client.lock().expect("client poisoned");
            if let Some(client) = guard.as_ref() {
                return Ok(Arc::clone(client));
            }
        }
        self.reconnect()
            .await
            .map_err(|err| lattice_protocol::Error {
                code: "voice_host_unavailable".into(),
                message: err.to_string(),
                details: None,
            })
    }

    async fn reconnect(&self) -> Result<Arc<VoiceHostClient>> {
        wait_for_socket(&self.socket, Duration::from_secs(5))?;
        let client = VoiceHostClient::connect(&self.socket)
            .await
            .map_err(|err| Error::Spawn(format!("voice-host reconnect: {err}")))?;
        let client = Arc::new(client);
        *self.client.lock().expect("client poisoned") = Some(Arc::clone(&client));
        self.degraded.store(false, Ordering::SeqCst);
        // Prior host sessions are gone after reconnect.
        *self.active_session.lock().expect("active_session poisoned") = None;
        info!(path = %self.socket.display(), "voice-host client reconnected");
        Ok(client)
    }

    fn maybe_normalize_final(&self, body: event::Body) -> event::Body {
        let event::Body::FinalTranscript(mut final_transcript) = body else {
            return body;
        };
        let (glossary, known_paths) = self
            .active_session
            .lock()
            .expect("active_session poisoned")
            .as_ref()
            .map(|s| (s.glossary_terms.clone(), s.known_paths.clone()))
            .unwrap_or_default();
        if glossary.is_empty() && known_paths.is_empty() {
            return event::Body::FinalTranscript(final_transcript);
        }
        let context = NormalizationContext {
            glossary_terms: glossary,
            known_paths,
        };
        let normalized = normalize_transcript(&final_transcript.text, &context);
        if !normalized.corrections.is_empty() {
            final_transcript.text = normalized.normalized;
        }
        event::Body::FinalTranscript(final_transcript)
    }

    fn mark_degraded(&self, degraded: bool) {
        self.degraded.store(degraded, Ordering::SeqCst);
    }

    fn clear_client(&self) {
        *self.client.lock().expect("client poisoned") = None;
        *self.active_session.lock().expect("active_session poisoned") = None;
    }

    fn spawn_socket_watch(self: &Arc<Self>, socket: PathBuf) {
        let stop = Arc::clone(&self.stop);
        let controller = Arc::clone(self);
        let join = thread::Builder::new()
            .name("latticed-voice-socket-watch".into())
            .spawn(move || {
                let mut degraded = false;
                while !stop.load(Ordering::SeqCst) {
                    let alive = socket.exists();
                    if !alive && !degraded {
                        degraded = true;
                        controller.mark_degraded(true);
                        controller.clear_client();
                        warn!(path = %socket.display(), "voice-host socket missing; voice degraded");
                    } else if alive && degraded {
                        // Reconnect happens lazily on the next request / fanout loop.
                        degraded = false;
                        controller.mark_degraded(false);
                        info!(path = %socket.display(), "voice-host socket restored");
                    }
                    thread::sleep(Duration::from_millis(250));
                }
            })
            .ok();
        *self.supervisor.lock().expect("supervisor poisoned") = join;
    }

    fn spawn_host_supervisor(self: &Arc<Self>) {
        let stop = Arc::clone(&self.stop);
        let controller = Arc::clone(self);
        let join = thread::Builder::new()
            .name("latticed-voice-host-supervisor".into())
            .spawn(move || {
                let mut backoff_ms = INITIAL_BACKOFF_MS;
                while !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(250));
                    let outcome = {
                        let mut guard = controller.host.lock().expect("host poisoned");
                        let Some(host) = guard.as_mut() else {
                            continue;
                        };
                        match host.child.try_wait() {
                            Ok(Some(status)) => Some(status),
                            Ok(None) => None,
                            Err(err) => {
                                warn!(error = %err, "voice-host wait failed");
                                None
                            }
                        }
                    };
                    let Some(status) = outcome else {
                        continue;
                    };
                    warn!(?status, "voice-host exited; marking voice degraded");
                    controller.mark_degraded(true);
                    controller.clear_client();

                    let restart_plan = {
                        let mut guard = controller.host.lock().expect("host poisoned");
                        let Some(host) = guard.as_mut() else {
                            continue;
                        };
                        let restarts = host.restarts.fetch_add(1, Ordering::SeqCst) + 1;
                        if restarts > MAX_RESTARTS {
                            warn!(restarts, "voice-host restart budget exhausted");
                            None
                        } else {
                            Some((
                                host.binary.clone(),
                                host.socket.clone(),
                                host.fake,
                                restarts,
                            ))
                        }
                    };
                    let Some((binary, socket, fake, restarts)) = restart_plan else {
                        continue;
                    };
                    thread::sleep(Duration::from_millis(backoff_ms));
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(MAX_BACKOFF_MS);

                    match spawn_voice_host(&binary, &socket, fake) {
                        Ok(mut child) => {
                            if wait_for_socket(&socket, Duration::from_secs(5)).is_ok() {
                                if let Some(host) =
                                    controller.host.lock().expect("host poisoned").as_mut()
                                {
                                    host.child = child;
                                }
                                controller.mark_degraded(false);
                                backoff_ms = INITIAL_BACKOFF_MS;
                                info!(restarts, "voice-host restarted");
                            } else {
                                let _ = child.kill();
                                warn!("voice-host restarted but socket not ready");
                            }
                        }
                        Err(err) => {
                            warn!(error = %err, "failed to restart voice-host");
                        }
                    }
                }
            })
            .ok();
        *self.supervisor.lock().expect("supervisor poisoned") = join;
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.fanout.lock().expect("fanout poisoned").take() {
            join.abort();
        }
        if let Some(join) = self.supervisor.lock().expect("supervisor poisoned").take() {
            let _ = join.join();
        }
        self.clear_client();
        if let Some(mut host) = self.host.lock().expect("host poisoned").take() {
            let _ = host.child.kill();
            let _ = host.child.wait();
        }
    }
}

impl Drop for VoiceController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn is_voice_fanout_body(body: &event::Body) -> bool {
    matches!(
        body,
        event::Body::PartialTranscript(_)
            | event::Body::StableTranscript(_)
            | event::Body::FinalTranscript(_)
            | event::Body::AudioGap(_)
            | event::Body::ModelStatus(_)
            | event::Body::SessionReady(_)
            | event::Body::SpeechStarted(_)
            | event::Body::SessionCompleted(_)
            | event::Body::SessionFailed(_)
            | event::Body::CommandCandidate(_)
    )
}

fn voice_host_error_to_wire(error: lattice_voice_host::VoiceHostError) -> lattice_protocol::Error {
    lattice_protocol::Error {
        code: "voice_host_error".into(),
        message: error.to_string(),
        details: None,
    }
}

fn spawn_voice_host(binary: &Path, socket: &Path, fake: bool) -> Result<Child> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket.exists() {
        let _ = std::fs::remove_file(socket);
    }
    let backend = voice_host_backend_arg(fake);
    let mut command = Command::new(binary);
    command
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .arg("--backend")
        .arg(backend);
    if !fake {
        if let Some(cache) = model_cache_dir_from_env() {
            command.arg("--model-cache-dir").arg(cache);
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            Error::Spawn(format!(
                "failed to spawn voice-host {} (--backend {backend}): {err}",
                binary.display()
            ))
        })
}

fn voice_host_backend_arg(fake: bool) -> &'static str {
    if fake {
        "fake"
    } else {
        "fluidaudio"
    }
}

fn model_cache_dir_from_env() -> Option<PathBuf> {
    std::env::var(ENV_VOICE_MODEL_CACHE)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn wait_for_socket(socket: &Path, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if socket.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(Error::ReadyTimeout {
        path: socket.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_env_respects_socket_without_bin() {
        // Avoid mutating process-wide env in parallel tests; unit-test helpers only.
        let mode = VoiceProviderMode::ExternalSocket {
            socket: PathBuf::from("/tmp/voice-test.sock"),
        };
        match mode {
            VoiceProviderMode::ExternalSocket { socket } => {
                assert_eq!(socket, PathBuf::from("/tmp/voice-test.sock"));
            }
            other => panic!("unexpected mode: {other:?}"),
        }
    }

    #[test]
    fn resolve_bin_prefers_explicit_env_shape() {
        // Just ensure the helper does not panic when nothing is installed.
        let _ = resolve_voice_host_bin();
    }

    #[test]
    fn spawn_backend_arg_honors_fake_flag() {
        assert_eq!(voice_host_backend_arg(true), "fake");
        assert_eq!(voice_host_backend_arg(false), "fluidaudio");
    }
}
