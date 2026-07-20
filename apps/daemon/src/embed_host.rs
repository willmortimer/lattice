//! Optional `lattice-embed-host` supervision for semantic indexing.
//!
//! Host modes (`ExternalSocket` / `SpawnHost`) use [`ReconnectableEmbedHostProvider`]
//! so embedding jobs call the host over UDS. When the host dies, sessions are
//! marked semantically degraded so hybrid search falls back to FTS. Restarts use
//! bounded exponential backoff and reconnect the provider.
//!
//! Production default discovers/spawns `lattice-embed-host`. In-process Fake is
//! only selected when `LATTICE_SEMANTIC_FAKE=1`. When the host binary cannot be
//! found, the controller starts in [`SemanticProviderMode::Unavailable`] so FTS
//! still works and enable reports a clear failure (never a silent Fake).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lattice_embed_host::{install_model, ReconnectableEmbedHostProvider};
use lattice_embedding::{
    pinned_model_is_ready, qwen3_embedding_install_dir, sha256_hex, DistanceMetric,
    EmbeddingProvider, EmbeddingSpecification, FakeEmbeddingProvider, ModelManifest,
    PoolingStrategy, MANIFEST_SCHEMA_VERSION,
};
use lattice_runtime::{
    IndexProgressPhase, LatticeRuntime, RuntimeEvent, RuntimeIndexProgress, SemanticStatus,
    SemanticStatusState, SemanticWorkerConfig, WorkspaceSession,
};
use tracing::{info, warn};

use crate::config::default_run_dir;
use crate::error::{Error, Result};

/// Environment variable naming an existing embed-host UDS path.
pub const ENV_EMBED_HOST_SOCKET: &str = "LATTICE_EMBED_HOST_SOCKET";
/// When set to a truthy value, use an in-process [`FakeEmbeddingProvider`].
pub const ENV_SEMANTIC_FAKE: &str = "LATTICE_SEMANTIC_FAKE";
/// Optional path to the `lattice-embed-host` binary to spawn.
pub const ENV_EMBED_HOST_BIN: &str = "LATTICE_EMBED_HOST_BIN";
/// Optional explicit host backend (`fake` or `llama-cpp`). When unset, prefers
/// `llama-cpp` when the pinned GGUF is ready and the binary lists that backend;
/// otherwise host `--backend fake` is used only as temporary bootstrap before
/// prepare + reload (never in-process Fake).
pub const ENV_EMBED_HOST_BACKEND: &str = "LATTICE_EMBED_HOST_BACKEND";

const MAX_RESTARTS: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_BACKOFF_MS: u64 = 5_000;
/// Dimensions used for the host fake-backend bootstrap fixture.
const HOST_FAKE_DIMENSIONS: u32 = 8;

const UNAVAILABLE_MESSAGE: &str =
    "lattice-embed-host is not available; install it or set LATTICE_EMBED_HOST_BIN. FTS search still works.";

/// How the daemon obtains an embedding provider.
#[derive(Debug, Clone)]
pub enum SemanticProviderMode {
    /// Deterministic in-process fake (tests / CI via `LATTICE_SEMANTIC_FAKE=1`).
    FakeInProcess,
    /// Connect to an already-running embed-host socket (supervision optional).
    ExternalSocket { socket: PathBuf },
    /// Spawn `lattice-embed-host` and supervise it.
    SpawnHost {
        binary: PathBuf,
        socket: PathBuf,
        models_dir: PathBuf,
    },
    /// Host binary missing; semantic enable fails clearly while FTS remains usable.
    Unavailable,
}

impl SemanticProviderMode {
    /// Resolve provider mode from environment variables.
    ///
    /// Returns [`None`] when no semantic env is set; callers should use
    /// [`Self::from_env_or_default`], which discovers `lattice-embed-host` or
    /// returns [`Self::Unavailable`]. Fake is selected only via
    /// [`ENV_SEMANTIC_FAKE`].
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

    /// Env override when present; otherwise discover/spawn the embed host, or
    /// [`Self::Unavailable`] when no binary can be found.
    ///
    /// Never silently defaults to [`Self::FakeInProcess`].
    pub fn from_env_or_default() -> Self {
        Self::resolve_default(Self::from_env, resolve_embed_host_bin)
    }

    /// Deprecated alias for [`Self::from_env_or_default`].
    #[deprecated(
        note = "use from_env_or_default; Fake is only selected via LATTICE_SEMANTIC_FAKE"
    )]
    pub fn from_env_or_fake() -> Self {
        Self::from_env_or_default()
    }

    fn resolve_default(
        from_env: impl FnOnce() -> Option<Self>,
        resolve_bin: impl FnOnce() -> Option<PathBuf>,
    ) -> Self {
        if let Some(mode) = from_env() {
            return mode;
        }
        match resolve_bin() {
            Some(binary) => Self::spawn_host_default(binary),
            None => Self::Unavailable,
        }
    }

    fn spawn_host_default(binary: PathBuf) -> Self {
        let run_dir = default_run_dir();
        Self::SpawnHost {
            binary,
            socket: run_dir.join("embed-host.sock"),
            models_dir: run_dir.join("embed-models"),
        }
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

/// Wire-facing embedding provider identity (Settings / GetSemanticStatus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity {
    pub provider_id: String,
    pub model_id: Option<String>,
    pub dimensions: Option<u32>,
}

/// Shared semantic indexing controller for a daemon instance.
pub struct SemanticController {
    runtime: Arc<LatticeRuntime>,
    /// Absent in [`SemanticProviderMode::Unavailable`] (never a silent Fake).
    provider: Option<Arc<dyn EmbeddingProvider>>,
    /// Present for host-backed modes so supervisors can reconnect after restart.
    host_provider: Option<Arc<ReconnectableEmbedHostProvider>>,
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
    backend: String,
    restarts: AtomicU32,
}

impl SemanticController {
    /// Build a controller for the given mode (connect/spawn/fake/unavailable).
    pub fn start(runtime: Arc<LatticeRuntime>, mode: SemanticProviderMode) -> Result<Arc<Self>> {
        match mode {
            SemanticProviderMode::FakeInProcess => {
                let provider: Arc<dyn EmbeddingProvider> =
                    Arc::new(FakeEmbeddingProvider::new(fake_specification()));
                Ok(Arc::new(Self {
                    runtime,
                    provider: Some(provider),
                    host_provider: None,
                    runtime_handle: None,
                    _owned_runtime: None,
                    host: Mutex::new(None),
                    sessions: Mutex::new(Vec::new()),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                }))
            }
            SemanticProviderMode::ExternalSocket { socket } => {
                let (handle, owned) = take_embed_runtime()?;
                wait_for_socket(&socket, Duration::from_secs(5))?;
                let models_dir = socket
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("embed-models");
                let (model_dir, dimensions) = ensure_host_model_dir(&models_dir)?;
                let host_provider =
                    connect_host_provider(&handle, &socket, &model_dir, dimensions)?;
                let provider: Arc<dyn EmbeddingProvider> =
                    host_provider.clone() as Arc<dyn EmbeddingProvider>;
                let controller = Arc::new(Self {
                    runtime,
                    provider: Some(provider),
                    host_provider: Some(host_provider),
                    runtime_handle: Some(handle),
                    _owned_runtime: owned,
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
                let (handle, owned) = take_embed_runtime()?;
                let (model_dir, dimensions) = ensure_host_model_dir(&models_dir)?;
                let backend = resolve_spawn_backend(&binary);
                let child = spawn_embed_host(&binary, &socket, &models_dir, &backend)?;
                wait_for_socket(&socket, Duration::from_secs(5))?;
                let host_provider =
                    connect_host_provider(&handle, &socket, &model_dir, dimensions)?;
                let provider: Arc<dyn EmbeddingProvider> =
                    host_provider.clone() as Arc<dyn EmbeddingProvider>;
                let controller = Arc::new(Self {
                    runtime,
                    provider: Some(provider),
                    host_provider: Some(host_provider),
                    runtime_handle: Some(handle),
                    _owned_runtime: owned,
                    host: Mutex::new(Some(SupervisedHost {
                        child,
                        socket: socket.clone(),
                        binary,
                        models_dir,
                        backend,
                        restarts: AtomicU32::new(0),
                    })),
                    sessions: Mutex::new(Vec::new()),
                    stop: Arc::new(AtomicBool::new(false)),
                    supervisor: Mutex::new(None),
                });
                controller.spawn_host_supervisor();
                Ok(controller)
            }
            SemanticProviderMode::Unavailable => Ok(Arc::new(Self {
                runtime,
                provider: None,
                host_provider: None,
                runtime_handle: None,
                _owned_runtime: None,
                host: Mutex::new(None),
                sessions: Mutex::new(Vec::new()),
                stop: Arc::new(AtomicBool::new(false)),
                supervisor: Mutex::new(None),
            })),
        }
    }

    /// Attach semantic indexing to a newly opened workspace session.
    ///
    /// No-op when the controller has no provider ([`SemanticProviderMode::Unavailable`]).
    pub fn attach_session(self: &Arc<Self>, session: &Arc<WorkspaceSession>) {
        let Some(provider) = self.provider.as_ref() else {
            return;
        };
        let mut config = SemanticWorkerConfig::new(Arc::clone(provider));
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

    /// Enable semantic indexing for an open workspace (user-driven).
    ///
    /// Acquires the pinned embedding model (unless Fake / already installed),
    /// then attaches the session worker. Progress is published on the session
    /// prepare status for GetSemanticStatus polling.
    ///
    /// When the embed host is missing, returns a Failed status (FTS still works).
    pub fn enable_workspace(
        self: &Arc<Self>,
        workspace_id: &str,
    ) -> std::result::Result<SemanticStatus, String> {
        let session = self
            .runtime
            .get_session_by_id(workspace_id)
            .ok_or_else(|| format!("workspace session not found for id {workspace_id}"))?;
        if self.provider.is_none() {
            let status = unavailable_status();
            session.set_semantic_prepare_status(Some(status.clone()));
            return Ok(status);
        }
        lattice_handlers::prepare_semantic_model_for_session(&session, &mut |_| {})?;
        self.reload_host_after_prepare()
            .map_err(|err| err.to_string())?;
        self.attach_session(&session);
        Ok(session.semantic_status())
    }

    /// After E5 prepare, load the verified pinned GGUF on the host provider.
    ///
    /// When SpawnHost was started on the fake fixture and the binary supports
    /// llama-cpp, restart the child with `--backend llama-cpp` before reload.
    fn reload_host_after_prepare(self: &Arc<Self>) -> Result<()> {
        let Some(host_provider) = self.host_provider.as_ref() else {
            return Ok(());
        };
        if !pinned_model_is_ready() {
            return Ok(());
        }
        let model_dir = qwen3_embedding_install_dir();
        if let Some(handle) = self.runtime_handle.as_ref() {
            self.maybe_restart_host_for_llama()?;
            block_on_embed_io(handle, host_provider.reload_model(model_dir, None))
                .map_err(|err| Error::Spawn(format!("reload pinned GGUF on embed-host: {err}")))?;
            info!("embed-host reloaded pinned Qwen3 GGUF after prepare");
        }
        Ok(())
    }

    fn maybe_restart_host_for_llama(self: &Arc<Self>) -> Result<()> {
        let restart = {
            let mut guard = self.host.lock().expect("host poisoned");
            let Some(host) = guard.as_mut() else {
                return Ok(());
            };
            if host.backend == "llama-cpp" {
                return Ok(());
            }
            if !binary_supports_backend(&host.binary, "llama-cpp") {
                return Ok(());
            }
            let _ = host.child.kill();
            let _ = host.child.wait();
            Some((
                host.binary.clone(),
                host.socket.clone(),
                host.models_dir.clone(),
            ))
        };
        let Some((binary, socket, models_dir)) = restart else {
            return Ok(());
        };
        let child = spawn_embed_host(&binary, &socket, &models_dir, "llama-cpp")?;
        wait_for_socket(&socket, Duration::from_secs(5))?;
        let mut guard = self.host.lock().expect("host poisoned");
        if let Some(host) = guard.as_mut() {
            host.child = child;
            host.backend = "llama-cpp".into();
            host.restarts.store(0, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Stop semantic indexing for a workspace (FTS remains available).
    pub fn disable_workspace(&self, workspace_id: &str) -> std::result::Result<SemanticStatus, String> {
        let session = self
            .runtime
            .get_session_by_id(workspace_id)
            .ok_or_else(|| format!("workspace session not found for id {workspace_id}"))?;
        session.stop_semantic_indexing();
        self.prune_sessions();
        Ok(SemanticStatus::stopped())
    }

    /// Status for a workspace session, or stopped when unknown / disabled.
    pub fn status_for_workspace(&self, workspace_id: &str) -> SemanticStatus {
        match self.runtime.get_session_by_id(workspace_id) {
            Some(session) => session.semantic_status(),
            None => SemanticStatus::stopped().with_message(format!(
                "workspace session not found for id {workspace_id}"
            )),
        }
    }

    fn prune_sessions(&self) {
        self.sessions
            .lock()
            .expect("sessions poisoned")
            .retain(|weak| weak.strong_count() > 0);
    }

    pub fn provider(&self) -> Option<Arc<dyn EmbeddingProvider>> {
        self.provider.as_ref().map(Arc::clone)
    }

    /// Active provider identity for GetSemanticStatus / wire enrichment.
    ///
    /// When no provider is attached ([`SemanticProviderMode::Unavailable`]),
    /// returns `provider_id = "unavailable"` with unset model/dimensions.
    pub fn provider_identity(&self) -> ProviderIdentity {
        match self.provider.as_ref() {
            Some(provider) => {
                let spec = provider.specification();
                ProviderIdentity {
                    provider_id: spec.provider_id.clone(),
                    model_id: Some(spec.model_id.clone()),
                    dimensions: Some(spec.dimensions),
                }
            }
            None => ProviderIdentity {
                provider_id: "unavailable".into(),
                model_id: None,
                dimensions: None,
            },
        }
    }

    /// True when jobs are backed by the embed-host UDS client (not in-process Fake).
    pub fn uses_host_provider(&self) -> bool {
        self.host_provider.is_some()
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

    fn reconnect_host_provider(&self) {
        let Some(host_provider) = self.host_provider.as_ref() else {
            return;
        };
        let Some(handle) = self.runtime_handle.as_ref() else {
            return;
        };
        match block_on_embed_io(handle, host_provider.reconnect()) {
            Ok(()) => info!("embed-host provider reconnected"),
            Err(err) => warn!(error = %err, "embed-host provider reconnect failed"),
        }
    }

    /// Kill the supervised embed-host child (integration tests only).
    #[doc(hidden)]
    pub fn kill_supervised_host_for_test(&self) -> bool {
        let mut guard = self.host.lock().expect("host poisoned");
        let Some(host) = guard.as_mut() else {
            return false;
        };
        let _ = host.child.kill();
        let _ = host.child.wait();
        // Prevent the supervisor from immediately restarting during the test.
        host.restarts.store(MAX_RESTARTS, Ordering::SeqCst);
        self.mark_all_degraded(true);
        true
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
                        controller.reconnect_host_provider();
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
                                host.backend.clone(),
                                restarts,
                            ))
                        }
                    };
                    let Some((binary, socket, models_dir, backend, restarts)) = restart_plan else {
                        continue;
                    };
                    thread::sleep(Duration::from_millis(backoff_ms));
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(MAX_BACKOFF_MS);

                    match spawn_embed_host(&binary, &socket, &models_dir, &backend) {
                        Ok(mut child) => {
                            if wait_for_socket(&socket, Duration::from_secs(5)).is_ok() {
                                if let Some(host) =
                                    controller.host.lock().expect("host poisoned").as_mut()
                                {
                                    host.child = child;
                                }
                                controller.reconnect_host_provider();
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

/// Prefer an ambient Tokio handle when starting from async `main`; otherwise
/// own a dedicated multi-thread runtime for host-backed providers.
fn take_embed_runtime() -> Result<(tokio::runtime::Handle, Option<tokio::runtime::Runtime>)> {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return Ok((handle, None));
    }
    let owned = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|err| Error::Spawn(format!("embed runtime: {err}")))?;
    let handle = owned.handle().clone();
    Ok((handle, Some(owned)))
}

fn block_on_embed_io<F, T>(handle: &tokio::runtime::Handle, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    // `Handle::block_on` panics when called from a worker already driving tasks.
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        handle.block_on(future)
    }
}

fn connect_host_provider(
    handle: &tokio::runtime::Handle,
    socket: &Path,
    model_dir: &Path,
    dimensions: Option<u32>,
) -> Result<Arc<ReconnectableEmbedHostProvider>> {
    block_on_embed_io(
        handle,
        ReconnectableEmbedHostProvider::connect(socket, model_dir, dimensions),
    )
    .map(Arc::new)
    .map_err(|err| Error::Spawn(format!("embed-host connect/load: {err}")))
}

/// Prefer a verified pinned install; otherwise stage a tiny fake fixture for the
/// host fake backend (CI / offline SpawnHost without a prepared GGUF).
///
/// Returns `(model_dir, optional load dimensions)`. `None` dimensions means the
/// host uses the manifest default (512 for pinned Qwen3).
fn ensure_host_model_dir(models_dir: &Path) -> Result<(PathBuf, Option<u32>)> {
    if pinned_model_is_ready() {
        return Ok((qwen3_embedding_install_dir(), None));
    }
    Ok((stage_fake_host_model(models_dir)?, Some(HOST_FAKE_DIMENSIONS)))
}

fn stage_fake_host_model(models_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(models_dir)?;
    let staging = models_dir.join(".staging-fake");
    std::fs::create_dir_all(&staging)?;
    let artifact_bytes = b"lattice-embed-host-fake-fixture";
    let sha = sha256_hex(artifact_bytes);
    let artifact = staging.join("fixture.bin");
    std::fs::write(&artifact, artifact_bytes)?;
    let manifest = ModelManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        provider: "fake".into(),
        model_id: "lattice/embed-host-fake".into(),
        model_revision: "e6".into(),
        artifact: "fixture.bin".into(),
        sha256: sha,
        license: "Apache-2.0".into(),
        native_dimensions: 32,
        default_dimensions: HOST_FAKE_DIMENSIONS,
        pooling: PoolingStrategy::Last,
        instruction_version: "lattice-retrieval-v1".into(),
    };
    let manifest_path = staging.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|err| Error::Spawn(err.to_string()))?,
    )?;
    let installed = install_model(&manifest_path, &artifact, models_dir)
        .map_err(|err| Error::Spawn(format!("stage fake host model: {err}")))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(installed.model_dir)
}

/// Prefer llama-cpp when the binary supports it and the pinned GGUF is ready.
/// Otherwise use host `--backend fake` only as temporary bootstrap before
/// prepare + reload. Never selects in-process [`FakeEmbeddingProvider`].
fn resolve_spawn_backend(binary: &Path) -> String {
    if let Ok(explicit) = std::env::var(ENV_EMBED_HOST_BACKEND) {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if binary_supports_backend(binary, "llama-cpp") && pinned_model_is_ready() {
        return "llama-cpp".into();
    }
    // Temporary bootstrap until GGUF prepare; reload_host_after_prepare switches.
    "fake".into()
}

fn binary_supports_backend(binary: &Path, backend: &str) -> bool {
    let output = Command::new(binary).arg("backends").output();
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines().any(|line| line.trim() == backend)
        }
        _ => false,
    }
}

fn spawn_embed_host(
    binary: &Path,
    socket: &Path,
    models_dir: &Path,
    backend: &str,
) -> Result<Child> {
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
        .arg(backend)
        .arg("--models-dir")
        .arg(models_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            Error::Spawn(format!(
                "failed to spawn embed-host {} (--backend {backend}): {err}",
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

fn unavailable_status() -> SemanticStatus {
    SemanticStatus {
        state: SemanticStatusState::Failed,
        pending_chunks: None,
        message: Some(UNAVAILABLE_MESSAGE.into()),
        progress_percent: None,
    }
}

fn current_exe_sibling(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}

/// Locate `lattice-embed-host` for production discovery / tests / local launches.
///
/// Order: `LATTICE_EMBED_HOST_BIN`, `PATH`, sibling of `current_exe` (macOS app
/// bundle `Contents/MacOS/`), then common cargo target dirs.
pub fn resolve_embed_host_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_EMBED_HOST_BIN) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(path) = which_bin("lattice-embed-host") {
        return Some(path);
    }

    if let Some(path) = current_exe_sibling("lattice-embed-host") {
        return Some(path);
    }

    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/lattice-embed-host"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/lattice-embed-host"),
        PathBuf::from("target/debug/lattice-embed-host"),
        PathBuf::from("target/release/lattice-embed-host"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use lattice_embedding::{EmbedQueryRequest, FakeEmbeddingProvider};
    use lattice_runtime::{
        hybrid_search_with_session_semantic, SemanticAvailability, SemanticStatusState,
    };
    use std::time::Instant;

    fn ensure_embed_host_bin() -> PathBuf {
        if let Some(path) = resolve_embed_host_bin() {
            return path;
        }
        let status = std::process::Command::new(env!("CARGO"))
            .args([
                "build",
                "-p",
                "lattice-embed-host",
                "--bin",
                "lattice-embed-host",
            ])
            .status()
            .expect("spawn cargo build lattice-embed-host");
        assert!(
            status.success(),
            "cargo build -p lattice-embed-host failed: {status}"
        );
        resolve_embed_host_bin().expect(
            "lattice-embed-host binary missing after build (set LATTICE_EMBED_HOST_BIN)",
        )
    }

    #[test]
    fn from_env_or_default_prefers_fake_env_over_discovery() {
        let mode = SemanticProviderMode::resolve_default(
            || Some(SemanticProviderMode::FakeInProcess),
            || Some(PathBuf::from("/tmp/would-not-use")),
        );
        assert!(
            matches!(mode, SemanticProviderMode::FakeInProcess),
            "LATTICE_SEMANTIC_FAKE / from_env Fake must win over binary discovery"
        );
    }

    #[test]
    fn from_env_or_default_without_binary_is_unavailable() {
        let mode = SemanticProviderMode::resolve_default(|| None, || None);
        assert!(matches!(mode, SemanticProviderMode::Unavailable));
    }

    #[test]
    fn from_env_or_default_discovers_spawn_host() {
        let mode = SemanticProviderMode::resolve_default(
            || None,
            || Some(PathBuf::from("/tmp/lattice-embed-host")),
        );
        match mode {
            SemanticProviderMode::SpawnHost {
                binary,
                socket,
                models_dir,
            } => {
                assert_eq!(binary, PathBuf::from("/tmp/lattice-embed-host"));
                assert!(socket.ends_with("embed-host.sock"));
                assert!(models_dir.ends_with("embed-models"));
            }
            other => panic!("expected SpawnHost, got {other:?}"),
        }
    }

    #[test]
    fn unavailable_enable_returns_failed_without_fake_provider() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Unavailable Semantic").unwrap();
        let runtime = Arc::new(LatticeRuntime::new());
        let controller =
            SemanticController::start(Arc::clone(&runtime), SemanticProviderMode::Unavailable)
                .unwrap();
        assert!(!controller.uses_host_provider());
        assert!(controller.provider().is_none());

        let session = runtime.open_workspace_session(dir.path()).unwrap();
        let workspace_id = session.workspace_id().to_string();
        controller.attach_session(&session);
        assert_eq!(
            session.semantic_status().state,
            SemanticStatusState::Stopped,
            "attach_session is a no-op when Unavailable"
        );

        let enabled = controller.enable_workspace(&workspace_id).unwrap();
        assert_eq!(enabled.state, SemanticStatusState::Failed);
        assert!(
            enabled
                .message
                .as_deref()
                .is_some_and(|m| m.contains("lattice-embed-host")),
            "expected missing-host message, got {:?}",
            enabled.message
        );
        assert_eq!(
            controller.status_for_workspace(&workspace_id).state,
            SemanticStatusState::Failed
        );

        controller.shutdown();
        runtime.close_session(dir.path()).unwrap();
    }

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

    #[test]
    fn enable_disable_updates_status_without_env_gate() {
        std::env::set_var(ENV_SEMANTIC_FAKE, "1");
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Daemon Enable").unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Notes\n\nhello semantic\n").unwrap();

        let runtime = Arc::new(LatticeRuntime::new());
        let controller =
            SemanticController::start(Arc::clone(&runtime), SemanticProviderMode::FakeInProcess)
                .unwrap();
        let session = runtime.open_workspace_session(dir.path()).unwrap();
        let workspace_id = session.workspace_id().to_string();

        assert_eq!(
            controller.status_for_workspace(&workspace_id).state,
            SemanticStatusState::Stopped
        );

        let enabled = controller.enable_workspace(&workspace_id).unwrap();
        assert_ne!(enabled.state, SemanticStatusState::Stopped);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let status = controller.status_for_workspace(&workspace_id);
            if matches!(
                status.state,
                SemanticStatusState::Ready | SemanticStatusState::Indexing
            ) {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        let disabled = controller.disable_workspace(&workspace_id).unwrap();
        assert_eq!(disabled.state, SemanticStatusState::Stopped);
        assert_eq!(
            controller.status_for_workspace(&workspace_id).state,
            SemanticStatusState::Stopped
        );

        controller.shutdown();
        runtime.close_session(dir.path()).unwrap();
        std::env::remove_var(ENV_SEMANTIC_FAKE);
    }

    #[test]
    fn spawn_host_provider_embeds_via_rpc_and_kills_to_degrade() {
        let bin = ensure_embed_host_bin();
        let host_dir = tempfile::tempdir().unwrap();
        let socket = host_dir.path().join("embed-host.sock");
        let models_dir = host_dir.path().join("embed-models");
        // Isolate from any user-installed pinned GGUF so we stage the fake fixture.
        let profile = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_DEV_HOME", profile.path());

        let workspace = tempfile::tempdir().unwrap();
        Workspace::init(workspace.path(), "Host Provider").unwrap();
        std::fs::write(
            workspace.path().join("Notes.md"),
            "# Notes\n\nCapability grants for plugins.\n",
        )
        .unwrap();

        let runtime = Arc::new(LatticeRuntime::new());
        let controller = SemanticController::start(
            Arc::clone(&runtime),
            SemanticProviderMode::SpawnHost {
                binary: bin,
                socket,
                models_dir,
            },
        )
        .unwrap();
        assert!(
            controller.uses_host_provider(),
            "SpawnHost must use EmbedHostClient-backed provider"
        );
        assert_eq!(
            controller.provider().expect("host provider").specification().dimensions,
            HOST_FAKE_DIMENSIONS,
            "host fake fixture dimensions must differ from in-process Fake (12)"
        );

        let session = runtime.open_workspace_session(workspace.path()).unwrap();
        controller.attach_session(&session);

        let deadline = Instant::now() + Duration::from_secs(10);
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
        assert!(
            session
                .semantic_namespace_id()
                .and_then(|ns| {
                    session
                        .index()
                        .chunk_embedding_states_for_namespace(ns)
                        .ok()
                })
                .map(|states| states.iter().any(|s| s.status.as_str() == "ready"))
                .unwrap_or(false),
            "host-backed worker should embed pending chunks"
        );

        // Vectors must match the host fake backend (manifest dims), not the
        // daemon in-process Fake specification (dims=12).
        let host_probe = {
            let handle = controller.runtime_handle.as_ref().unwrap();
            block_on_embed_io(handle, async {
                controller
                    .provider()
                    .expect("host provider")
                    .embed_query(EmbedQueryRequest {
                        text: "capability grants".into(),
                    })
                    .await
                    .unwrap()
            })
        };
        let local_fake = FakeEmbeddingProvider::new(fake_specification());
        let local_probe = {
            let handle = controller.runtime_handle.as_ref().unwrap();
            block_on_embed_io(handle, async {
                local_fake
                    .embed_query(EmbedQueryRequest {
                        text: "capability grants".into(),
                    })
                    .await
                    .unwrap()
            })
        };
        assert_ne!(
            host_probe.values.len(),
            local_probe.values.len(),
            "host RPC vectors must not come from in-process Fake dims"
        );

        let hits =
            hybrid_search_with_session_semantic(&session, "capability grants", 10).unwrap();
        assert!(hits.iter().any(|h| h.semantic_rank.is_some()));

        assert!(controller.kill_supervised_host_for_test());
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
        runtime.close_session(workspace.path()).unwrap();
        std::env::remove_var("LATTICE_DEV_HOME");
    }
}
