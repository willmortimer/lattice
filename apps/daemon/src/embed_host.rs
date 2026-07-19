//! Optional `lattice-embed-host` supervision for semantic indexing.
//!
//! When the host dies, sessions are marked semantically degraded so hybrid
//! search falls back to FTS. Restarts use bounded exponential backoff.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lattice_embedding::{
    DistanceMetric, EmbeddingProvider, EmbeddingSpecification, FakeEmbeddingProvider,
    PoolingStrategy,
};
use lattice_runtime::{
    IndexProgressPhase, LatticeRuntime, RuntimeEvent, RuntimeIndexProgress, SemanticWorkerConfig,
    WorkspaceSession,
};
use tracing::{info, warn};

use crate::error::{Error, Result};

/// Environment variable naming an existing embed-host UDS path.
pub const ENV_EMBED_HOST_SOCKET: &str = "LATTICE_EMBED_HOST_SOCKET";
/// When set to a truthy value, use an in-process [`FakeEmbeddingProvider`].
pub const ENV_SEMANTIC_FAKE: &str = "LATTICE_SEMANTIC_FAKE";
/// Optional path to the `lattice-embed-host` binary to spawn.
pub const ENV_EMBED_HOST_BIN: &str = "LATTICE_EMBED_HOST_BIN";

const MAX_RESTARTS: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_BACKOFF_MS: u64 = 5_000;

/// How the daemon obtains an embedding provider.
#[derive(Debug, Clone)]
pub enum SemanticProviderMode {
    /// Deterministic in-process fake (tests / CI).
    FakeInProcess,
    /// Connect to an already-running embed-host socket (supervision optional).
    ExternalSocket { socket: PathBuf },
    /// Spawn `lattice-embed-host` and supervise it.
    SpawnHost {
        binary: PathBuf,
        socket: PathBuf,
        models_dir: PathBuf,
    },
}

impl SemanticProviderMode {
    /// Resolve provider mode from environment variables.
    pub fn from_env() -> Option<Self> {
        if env_truthy(ENV_SEMANTIC_FAKE) {
            return Some(Self::FakeInProcess);
        }
        if let Ok(socket) = std::env::var(ENV_EMBED_HOST_SOCKET) {
            let socket = PathBuf::from(socket);
            if let Ok(bin) = std::env::var(ENV_EMBED_HOST_BIN) {
                let models_dir = socket
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("embed-models");
                return Some(Self::SpawnHost {
                    binary: PathBuf::from(bin),
                    socket,
                    models_dir,
                });
            }
            return Some(Self::ExternalSocket { socket });
        }
        None
    }
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn fake_specification() -> EmbeddingSpecification {
    EmbeddingSpecification {
        provider_id: "fake".into(),
        model_id: "fake-model".into(),
        model_revision: "rev-1".into(),
        artifact_sha256: "sha256:fake".into(),
        dimensions: 12,
        native_dimensions: 12,
        distance: DistanceMetric::Cosine,
        pooling: PoolingStrategy::Last,
        normalized: true,
        instruction_version: "daemon-fake-v1".into(),
    }
}

/// Shared semantic indexing controller for a daemon instance.
pub struct SemanticController {
    runtime: Arc<LatticeRuntime>,
    provider: Arc<dyn EmbeddingProvider>,
    runtime_handle: Option<tokio::runtime::Handle>,
    /// Owns the multi-thread runtime used by host-backed providers.
    _owned_runtime: Option<tokio::runtime::Runtime>,
    host: Mutex<Option<SupervisedHost>>,
    sessions: Mutex<Vec<Weak<WorkspaceSession>>>,
    stop: Arc<AtomicBool>,
    supervisor: Mutex<Option<JoinHandle<()>>>,
}

struct SupervisedHost {
    child: Child,
    socket: PathBuf,
    binary: PathBuf,
    models_dir: PathBuf,
    restarts: AtomicU32,
}

impl SemanticController {
    /// Build a controller for the given mode (connect/spawn/fake).
    pub fn start(runtime: Arc<LatticeRuntime>, mode: SemanticProviderMode) -> Result<Arc<Self>> {
        match mode {
            SemanticProviderMode::FakeInProcess => {
                let provider: Arc<dyn EmbeddingProvider> =
                    Arc::new(FakeEmbeddingProvider::new(fake_specification()));
                Ok(Arc::new(Self {
                    runtime,
                    provider,
                    runtime_handle: None,
                    _owned_runtime: None,
                    host: Mutex::new(None),
                    sessions: Mutex::new(Vec::new()),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                }))
            }
            SemanticProviderMode::ExternalSocket { socket } => {
                let owned = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .map_err(|err| Error::Spawn(format!("embed runtime: {err}")))?;
                let handle = owned.handle().clone();
                // Job processing uses the in-process fake; socket watch marks
                // degraded when the external host disappears (FTS still works).
                let provider: Arc<dyn EmbeddingProvider> =
                    Arc::new(FakeEmbeddingProvider::new(fake_specification()));
                let controller = Arc::new(Self {
                    runtime,
                    provider,
                    runtime_handle: Some(handle),
                    _owned_runtime: Some(owned),
                    host: Mutex::new(None),
                    sessions: Mutex::new(Vec::new()),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                });
                controller.spawn_socket_watch(socket);
                Ok(controller)
            }
            SemanticProviderMode::SpawnHost {
                binary,
                socket,
                models_dir,
            } => {
                let owned = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .map_err(|err| Error::Spawn(format!("embed runtime: {err}")))?;
                let handle = owned.handle().clone();
                let child = spawn_embed_host(&binary, &socket, &models_dir)?;
                wait_for_socket(&socket, Duration::from_secs(5))?;
                let provider: Arc<dyn EmbeddingProvider> =
                    Arc::new(FakeEmbeddingProvider::new(fake_specification()));
                let controller = Arc::new(Self {
                    runtime,
                    provider,
                    runtime_handle: Some(handle),
                    _owned_runtime: Some(owned),
                    host: Mutex::new(Some(SupervisedHost {
                        child,
                        socket: socket.clone(),
                        binary,
                        models_dir,
                        restarts: AtomicU32::new(0),
                    })),
                    sessions: Mutex::new(Vec::new()),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                });
                controller.spawn_host_supervisor();
                Ok(controller)
            }
        }
    }

    /// Attach semantic indexing to a newly opened workspace session.
    pub fn attach_session(self: &Arc<Self>, session: &Arc<WorkspaceSession>) {
        let mut config = SemanticWorkerConfig::new(Arc::clone(&self.provider));
        if let Some(handle) = self.runtime_handle.clone() {
            config = config.with_runtime_handle(handle);
        }
        if let Err(err) = session.start_semantic_indexing(Arc::clone(self.runtime.events()), config)
        {
            warn!(error = %err, "failed to start semantic indexing for session");
            return;
        }
        self.sessions
            .lock()
            .expect("sessions poisoned")
            .push(Arc::downgrade(session));
    }

    pub fn provider(&self) -> Arc<dyn EmbeddingProvider> {
        Arc::clone(&self.provider)
    }

    pub fn mark_all_degraded(&self, degraded: bool) {
        let mut sessions = self.sessions.lock().expect("sessions poisoned");
        sessions.retain(|weak| weak.strong_count() > 0);
        for weak in sessions.iter() {
            if let Some(session) = weak.upgrade() {
                self.publish_session_degraded(&session, degraded);
            }
        }
    }

    pub fn set_session_degraded(&self, session: &WorkspaceSession, degraded: bool) {
        self.publish_session_degraded(session, degraded);
    }

    fn publish_session_degraded(&self, session: &WorkspaceSession, degraded: bool) {
        session.set_semantic_degraded(degraded);
        let workspace_id = session.workspace_id().to_string();
        let phase = if degraded {
            IndexProgressPhase::SemanticDegraded
        } else {
            IndexProgressPhase::SemanticReady
        };
        self.runtime
            .events()
            .publish(RuntimeEvent::IndexProgress(RuntimeIndexProgress {
                workspace_id,
                phase,
                path: None,
                detail: Some(if degraded {
                    "embed host unavailable".into()
                } else {
                    "embed host ready".into()
                }),
            }));
    }

    fn spawn_socket_watch(self: &Arc<Self>, socket: PathBuf) {
        let stop = Arc::clone(&self.stop);
        let controller = Arc::clone(self);
        let join = thread::Builder::new()
            .name("latticed-embed-socket-watch".into())
            .spawn(move || {
                let mut degraded = false;
                while !stop.load(Ordering::SeqCst) {
                    let alive = socket.exists();
                    if !alive && !degraded {
                        degraded = true;
                        controller.mark_all_degraded(true);
                        warn!(path = %socket.display(), "embed-host socket missing; semantic degraded");
                    } else if alive && degraded {
                        degraded = false;
                        controller.mark_all_degraded(false);
                        info!(path = %socket.display(), "embed-host socket restored");
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
            .name("latticed-embed-host-supervisor".into())
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
                                warn!(error = %err, "embed-host wait failed");
                                None
                            }
                        }
                    };
                    let Some(status) = outcome else {
                        continue;
                    };
                    warn!(?status, "embed-host exited; marking semantic degraded");
                    controller.mark_all_degraded(true);

                    let restart_plan = {
                        let mut guard = controller.host.lock().expect("host poisoned");
                        let Some(host) = guard.as_mut() else {
                            continue;
                        };
                        let restarts = host.restarts.fetch_add(1, Ordering::SeqCst) + 1;
                        if restarts > MAX_RESTARTS {
                            warn!(restarts, "embed-host restart budget exhausted");
                            None
                        } else {
                            Some((
                                host.binary.clone(),
                                host.socket.clone(),
                                host.models_dir.clone(),
                                restarts,
                            ))
                        }
                    };
                    let Some((binary, socket, models_dir, restarts)) = restart_plan else {
                        continue;
                    };
                    thread::sleep(Duration::from_millis(backoff_ms));
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(MAX_BACKOFF_MS);

                    match spawn_embed_host(&binary, &socket, &models_dir) {
                        Ok(mut child) => {
                            if wait_for_socket(&socket, Duration::from_secs(5)).is_ok() {
                                if let Some(host) =
                                    controller.host.lock().expect("host poisoned").as_mut()
                                {
                                    host.child = child;
                                }
                                controller.mark_all_degraded(false);
                                backoff_ms = INITIAL_BACKOFF_MS;
                                info!(restarts, "embed-host restarted");
                            } else {
                                let _ = child.kill();
                                warn!("embed-host restarted but socket not ready");
                            }
                        }
                        Err(err) => {
                            warn!(error = %err, "failed to restart embed-host");
                        }
                    }
                }
            })
            .ok();
        *self.supervisor.lock().expect("supervisor poisoned") = join;
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.supervisor.lock().expect("supervisor poisoned").take() {
            let _ = join.join();
        }
        if let Some(mut host) = self.host.lock().expect("host poisoned").take() {
            let _ = host.child.kill();
            let _ = host.child.wait();
        }
    }
}

impl Drop for SemanticController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn spawn_embed_host(binary: &Path, socket: &Path, models_dir: &Path) -> Result<Child> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(models_dir)?;
    if socket.exists() {
        let _ = std::fs::remove_file(socket);
    }
    Command::new(binary)
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .arg("--backend")
        .arg("fake")
        .arg("--models-dir")
        .arg(models_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            Error::Spawn(format!(
                "failed to spawn embed-host {}: {err}",
                binary.display()
            ))
        })
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
    use lattice_core::Workspace;
    use lattice_runtime::{hybrid_search_with_session_semantic, SemanticAvailability};
    use std::time::Instant;

    #[test]
    fn fake_controller_embeds_and_degraded_fts_fallback() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Daemon Semantic").unwrap();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Notes\n\nCapability grants for plugins.\n",
        )
        .unwrap();

        let runtime = Arc::new(LatticeRuntime::new());
        let controller =
            SemanticController::start(Arc::clone(&runtime), SemanticProviderMode::FakeInProcess)
                .unwrap();
        let session = runtime.open_workspace_session(dir.path()).unwrap();
        controller.attach_session(&session);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(ns) = session.semantic_namespace_id() {
                let states = session
                    .index()
                    .chunk_embedding_states_for_namespace(ns)
                    .unwrap();
                if states.iter().any(|s| s.status.as_str() == "ready") {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }

        let hits = hybrid_search_with_session_semantic(&session, "capability grants", 10).unwrap();
        assert!(hits.iter().any(|h| h.semantic_rank.is_some()));

        controller.set_session_degraded(&session, true);
        assert_eq!(
            session.semantic_availability(),
            Some(SemanticAvailability::Degraded)
        );
        let fallback =
            hybrid_search_with_session_semantic(&session, "capability grants", 10).unwrap();
        assert!(fallback
            .iter()
            .any(|h| h.resource_uri.ends_with("Notes.md")));
        assert!(fallback.iter().all(|h| h.semantic_rank.is_none()));

        controller.shutdown();
        runtime.close_session(dir.path()).unwrap();
    }
}
