//! Optional Cell Apple VZ lab supervision (`cell-host-macos` + `celld`).
//!
//! Gated by `LATTICE_CELL_VZ=1`. Spawns and supervises the VZ helper socket and
//! a local `celld --backend=vz` for Lattice desktop demos without manual scripts.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tracing::{info, warn};

use crate::cell_vz_client::{
    observed_state_up, ping_payload_ok, CelldClient, CelldClientError, DEFAULT_CELL_ID,
};
use crate::config::default_run_dir;
use crate::error::{Error, Result};

/// Gate: enable Cell VZ supervision when set to a truthy value.
pub const ENV_CELL_VZ: &str = "LATTICE_CELL_VZ";
/// Optional path to `cell-host-macos`.
pub const ENV_CELL_HOST_BIN: &str = "LATTICE_CELL_HOST_BIN";
/// Optional path to `celld`.
pub const ENV_CELLD_BIN: &str = "LATTICE_CELLD_BIN";
/// Stable cell id for the supervised lattice-runtime guest.
pub const ENV_CELL_ID: &str = "LATTICE_CELL_ID";
/// Ping wait timeout for the background lattice loop (seconds).
pub const ENV_CELL_PING_TIMEOUT_SECS: &str = "LATTICE_CELL_PING_TIMEOUT_SECS";
/// Staged aarch64 CellOS artifacts (rootfs, kernel, initrd).
pub const ENV_CELL_VZ_IMAGES_DIR: &str = "CELL_VZ_IMAGES_DIR";
/// Unix socket served by `cell-host-macos`.
pub const ENV_CELL_VZ_HELPER_SOCKET: &str = "CELL_VZ_HELPER_SOCKET";
/// Persistent `celld` data directory (state DB, cell specs).
pub const ENV_CELL_DATA_DIR: &str = "LATTICE_CELL_DATA_DIR";
/// `celld --listen` address (default `127.0.0.1:18788`).
pub const ENV_CELL_LISTEN: &str = "LATTICE_CELL_LISTEN";

const DEFAULT_LISTEN: &str = "127.0.0.1:18788";
const SOCKET_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const CELLD_READY_TIMEOUT: Duration = Duration::from_secs(30);
const READY_POLL: Duration = Duration::from_millis(50);
const DEFAULT_PING_TIMEOUT: Duration = Duration::from_secs(300);
const PING_RETRY: Duration = Duration::from_secs(1);

/// Snapshot returned by [`CellVzController::status_snapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellVzStatus {
    pub up: bool,
    pub ping_ok: bool,
    pub phase: Option<String>,
    pub services_json: Option<String>,
    pub error: Option<String>,
}

impl CellVzStatus {
    pub fn gate_off(message: impl Into<String>) -> Self {
        Self {
            up: false,
            ping_ok: false,
            phase: None,
            services_json: None,
            error: Some(message.into()),
        }
    }
}

/// Resolved Cell VZ supervision configuration.
#[derive(Debug, Clone)]
pub struct CellVzConfig {
    pub host_bin: PathBuf,
    pub celld_bin: PathBuf,
    pub helper_socket: PathBuf,
    pub images_dir: PathBuf,
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub cell_id: String,
    pub ping_timeout: Duration,
}

/// How the daemon supervises Cell VZ children.
#[derive(Debug, Clone)]
pub enum CellVzProviderMode {
    Supervised(CellVzConfig),
}

impl CellVzProviderMode {
    /// Resolve when `LATTICE_CELL_VZ` is truthy; otherwise disabled.
    pub fn from_env() -> Option<Self> {
        if !env_truthy(ENV_CELL_VZ) {
            return None;
        }
        Some(Self::Supervised(resolve_config()))
    }
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn resolve_config() -> CellVzConfig {
    let host_bin = resolve_cell_host_bin().unwrap_or_else(|| PathBuf::from("cell-host-macos"));
    let celld_bin = resolve_celld_bin().unwrap_or_else(|| PathBuf::from("celld"));
    let helper_socket = std::env::var(ENV_CELL_VZ_HELPER_SOCKET)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_helper_socket_path);
    let images_dir = std::env::var(ENV_CELL_VZ_IMAGES_DIR)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_images_dir);
    let data_dir = std::env::var(ENV_CELL_DATA_DIR)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_cell_data_dir);
    let listen = std::env::var(ENV_CELL_LISTEN)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_LISTEN.to_string())
        .parse()
        .unwrap_or_else(|_| DEFAULT_LISTEN.parse().expect("default listen parses"));
    let cell_id = std::env::var(ENV_CELL_ID)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_CELL_ID.to_string());
    let ping_timeout = std::env::var(ENV_CELL_PING_TIMEOUT_SECS)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_PING_TIMEOUT);

    CellVzConfig {
        host_bin,
        celld_bin,
        helper_socket,
        images_dir,
        data_dir,
        listen,
        cell_id,
        ping_timeout,
    }
}

fn default_helper_socket_path() -> PathBuf {
    default_run_dir().join("cell-host-macos.sock")
}

fn default_images_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".cell")
        .join("images")
        .join("cellos-aarch64")
}

fn default_cell_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join("cell-lab")
}

/// Locate `cell-host-macos` for supervised launches.
pub fn resolve_cell_host_bin() -> Option<PathBuf> {
    resolve_named_bin(ENV_CELL_HOST_BIN, "cell-host-macos")
}

/// Locate `celld` for supervised launches.
pub fn resolve_celld_bin() -> Option<PathBuf> {
    resolve_named_bin(ENV_CELLD_BIN, "celld")
}

fn resolve_named_bin(env_key: &str, fallback_name: &str) -> Option<PathBuf> {
    if let Ok(path) = std::env::var(env_key) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(path) = which_bin(fallback_name) {
        return Some(path);
    }
    current_exe_sibling(fallback_name)
}

fn which_bin(name: &str) -> std::io::Result<PathBuf> {
    let path = std::env::var_os("PATH")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "PATH not set"))?;
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

struct SupervisedChildren {
    host: Child,
    celld: Child,
}

/// Supervises `cell-host-macos` and `celld` for the VZ lab path.
pub struct CellVzController {
    config: CellVzConfig,
    children: Mutex<Option<SupervisedChildren>>,
    stop: Arc<AtomicBool>,
    supervisor: Mutex<Option<JoinHandle<()>>>,
    lattice_loop: Mutex<Option<JoinHandle<()>>>,
    status: Mutex<CellVzStatus>,
    degraded: AtomicBool,
}

impl CellVzController {
    /// Spawn supervised children; fails fast when binaries are missing.
    pub async fn start(mode: CellVzProviderMode) -> Result<Arc<Self>> {
        let CellVzProviderMode::Supervised(config) = mode;
        validate_binary(&config.host_bin, "cell-host-macos", ENV_CELL_HOST_BIN)?;
        validate_binary(&config.celld_bin, "celld", ENV_CELLD_BIN)?;

        std::fs::create_dir_all(&config.data_dir)?;
        let state_path = config.data_dir.join("state").join("devcell.db");
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let host_child = spawn_cell_host(&config)?;
        wait_for_socket(&config.helper_socket, SOCKET_WAIT_TIMEOUT)?;

        let celld_child = spawn_celld(&config, &state_path)?;
        wait_for_celld(&config.listen, CELLD_READY_TIMEOUT)?;

        info!(
            helper_socket = %config.helper_socket.display(),
            listen = %config.listen,
            "cell VZ supervision ready"
        );

        let controller = Arc::new(Self {
            config: config.clone(),
            children: Mutex::new(Some(SupervisedChildren {
                host: host_child,
                celld: celld_child,
            })),
            stop: Arc::new(AtomicBool::new(false)),
            supervisor: Mutex::new(None),
            lattice_loop: Mutex::new(None),
            status: Mutex::new(CellVzStatus {
                up: true,
                ping_ok: false,
                phase: Some("celld_ready".into()),
                services_json: None,
                error: None,
            }),
            degraded: AtomicBool::new(false),
        });
        controller.spawn_supervisor();
        controller.spawn_lattice_loop();
        Ok(controller)
    }

    /// Non-blocking status for Settings / `cell_status` polling.
    pub fn status_snapshot(&self) -> CellVzStatus {
        self.status.lock().expect("status poisoned").clone()
    }

    fn set_status(&self, next: CellVzStatus) {
        *self.status.lock().expect("status poisoned") = next;
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::SeqCst)
    }

    pub fn helper_socket(&self) -> &Path {
        &self.config.helper_socket
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.config.listen
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.supervisor.lock().expect("supervisor poisoned").take() {
            let _ = join.join();
        }
        if let Some(join) = self.lattice_loop.lock().expect("lattice loop poisoned").take() {
            let _ = join.join();
        }
        if let Some(mut children) = self.children.lock().expect("children poisoned").take() {
            let _ = children.celld.kill();
            let _ = children.celld.wait();
            let _ = children.host.kill();
            let _ = children.host.wait();
        }
    }

    fn spawn_supervisor(self: &Arc<Self>) {
        let stop = Arc::clone(&self.stop);
        let controller = Arc::clone(self);
        let join = thread::Builder::new()
            .name("latticed-cell-vz-supervisor".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(500));
                    let exited = {
                        let mut guard = controller.children.lock().expect("children poisoned");
                        let Some(children) = guard.as_mut() else {
                            continue;
                        };
                        let host_exited = children.host.try_wait().ok().flatten().is_some();
                        let celld_exited = children.celld.try_wait().ok().flatten().is_some();
                        host_exited || celld_exited
                    };
                    if exited {
                        controller.degraded.store(true, Ordering::SeqCst);
                        warn!("cell VZ child exited; lab plane degraded");
                    }
                }
            })
            .ok();
        *self.supervisor.lock().expect("supervisor poisoned") = join;
    }

    fn spawn_lattice_loop(self: &Arc<Self>) {
        let stop = Arc::clone(&self.stop);
        let controller = Arc::clone(self);
        let join = thread::Builder::new()
            .name("latticed-cell-vz-lattice-loop".into())
            .spawn(move || controller.run_lattice_loop(&stop))
            .ok();
        *self.lattice_loop.lock().expect("lattice loop poisoned") = join;
    }

    fn run_lattice_loop(self: &Arc<Self>, stop: &AtomicBool) {
        let client = CelldClient::new(self.config.listen, self.config.cell_id.clone());
        if !has_vz_artifacts(&self.config.images_dir) {
            self.set_status(CellVzStatus {
                up: client.healthz_ok() && !self.is_degraded(),
                ping_ok: false,
                phase: Some("artifacts_missing".into()),
                services_json: None,
                error: Some(format!(
                    "VZ artifacts missing under {} (need cellos.ext4 and vmlinux/Image)",
                    self.config.images_dir.display()
                )),
            });
            return;
        }

        self.set_status(CellVzStatus {
            up: true,
            ping_ok: false,
            phase: Some("applying".into()),
            services_json: None,
            error: None,
        });
        if let Err(err) = client.apply_lattice_cell() {
            self.fail_status("apply", err);
            return;
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }

        self.set_status(CellVzStatus {
            up: true,
            ping_ok: false,
            phase: Some("starting".into()),
            services_json: None,
            error: None,
        });
        match client.get_observed_state() {
            Ok(Some(state)) if observed_state_up(&state) => {}
            Ok(_) => {
                if let Err(err) = client.start_cell() {
                    self.fail_status("start", err);
                    return;
                }
            }
            Err(err) => {
                if let Err(start_err) = client.start_cell() {
                    self.fail_status("start", start_err);
                    return;
                }
                warn!(error = %err, "GetCell before start failed; continuing after StartCell");
            }
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }

        self.set_status(CellVzStatus {
            up: true,
            ping_ok: false,
            phase: Some("pinging".into()),
            services_json: None,
            error: None,
        });
        let deadline = std::time::Instant::now() + self.config.ping_timeout;
        while !stop.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            match client.invoke_lattice_ping() {
                Ok(payload) if ping_payload_ok(&payload) => {
                    let services_json = serde_json::to_string(&payload).ok();
                    self.set_status(CellVzStatus {
                        up: true,
                        ping_ok: true,
                        phase: Some("ready".into()),
                        services_json,
                        error: None,
                    });
                    info!(cell_id = %self.config.cell_id, "lattice.runtime.v1 Ping OK");
                    return;
                }
                Ok(payload) => {
                    warn!(?payload, "Ping returned unexpected payload");
                }
                Err(err) => {
                    debug_ping_wait(&err);
                }
            }
            thread::sleep(PING_RETRY);
        }

        if stop.load(Ordering::SeqCst) {
            return;
        }
        self.set_status(CellVzStatus {
            up: true,
            ping_ok: false,
            phase: Some("ping_timeout".into()),
            services_json: None,
            error: Some(format!(
                "lattice.runtime.v1 Ping did not succeed within {}s",
                self.config.ping_timeout.as_secs()
            )),
        });
    }

    fn fail_status(&self, phase: &str, err: CelldClientError) {
        warn!(phase, error = %err, "cell VZ lattice loop step failed");
        self.set_status(CellVzStatus {
            up: !self.is_degraded(),
            ping_ok: false,
            phase: Some(phase.into()),
            services_json: None,
            error: Some(err.to_string()),
        });
    }
}

impl Drop for CellVzController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn validate_binary(path: &Path, display_name: &str, env_key: &str) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }
    Err(Error::Spawn(format!(
        "Cell VZ enabled ({ENV_CELL_VZ}=1) but {display_name} not found at {} \
         (set {env_key} or install {display_name} on PATH)",
        path.display()
    )))
}

fn spawn_cell_host(config: &CellVzConfig) -> Result<Child> {
    if let Some(parent) = config.helper_socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if config.helper_socket.exists() {
        let _ = std::fs::remove_file(&config.helper_socket);
    }

    Command::new(&config.host_bin)
        .arg("--socket")
        .arg(&config.helper_socket)
        .env(ENV_CELL_VZ_HELPER_SOCKET, &config.helper_socket)
        .env(ENV_CELL_VZ_IMAGES_DIR, &config.images_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            Error::Spawn(format!(
                "failed to spawn cell-host-macos {}: {err}",
                config.host_bin.display()
            ))
        })
}

fn spawn_celld(config: &CellVzConfig, state_path: &Path) -> Result<Child> {
    let mut command = Command::new(&config.celld_bin);
    command
        .arg("--http-dev")
        .arg("--listen")
        .arg(config.listen.to_string())
        .arg("--data-dir")
        .arg(&config.data_dir)
        .arg("--state")
        .arg(state_path)
        .arg("--backend=vz")
        .arg("--vz-helper-socket")
        .arg(&config.helper_socket)
        .env(ENV_CELL_VZ_HELPER_SOCKET, &config.helper_socket)
        .env(ENV_CELL_VZ_IMAGES_DIR, &config.images_dir);

    append_vz_artifact_args(&mut command, &config.images_dir);

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            Error::Spawn(format!(
                "failed to spawn celld {}: {err}",
                config.celld_bin.display()
            ))
        })
}

fn append_vz_artifact_args(command: &mut Command, images_dir: &Path) {
    let rootfs = images_dir.join("cellos.ext4");
    if !rootfs.is_file() {
        return;
    }
    command.arg("--vz-rootfs").arg(rootfs);
    let vmlinux = images_dir.join("vmlinux");
    let image = images_dir.join("Image");
    if vmlinux.is_file() {
        command.arg("--vz-kernel").arg(vmlinux);
    } else if image.is_file() {
        command.arg("--vz-kernel").arg(image);
    }
    let initrd = images_dir.join("initrd");
    if initrd.is_file() {
        command.arg("--vz-initrd").arg(initrd);
    }
}

fn wait_for_socket(socket: &Path, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if socket.exists() {
            return Ok(());
        }
        thread::sleep(READY_POLL);
    }
    Err(Error::ReadyTimeout {
        path: socket.display().to_string(),
    })
}

fn wait_for_celld(listen: &SocketAddr, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if TcpStream::connect_timeout(listen, Duration::from_millis(200)).is_ok() {
            return Ok(());
        }
        thread::sleep(READY_POLL);
    }
    Err(Error::Spawn(format!(
        "timed out waiting for celld to listen on {listen} \
         (check logs; ensure celld starts with --backend=vz)"
    )))
}

fn has_vz_artifacts(images_dir: &Path) -> bool {
    let rootfs = images_dir.join("cellos.ext4");
    if !rootfs.is_file() {
        return false;
    }
    images_dir.join("vmlinux").is_file() || images_dir.join("Image").is_file()
}

fn debug_ping_wait(err: &CelldClientError) {
    // Ping failures are expected while the guest boots; keep logs quiet.
    let _ = err;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_disabled_without_gate() {
        if std::env::var(ENV_CELL_VZ).is_ok() {
            return;
        }
        assert!(CellVzProviderMode::from_env().is_none());
    }

    #[test]
    fn default_paths_are_populated() {
        let config = resolve_config();
        assert!(!config.helper_socket.as_os_str().is_empty());
        assert!(!config.images_dir.as_os_str().is_empty());
        assert!(!config.data_dir.as_os_str().is_empty());
        assert_eq!(config.listen.port(), 18788);
    }

    #[test]
    fn gate_off_status_shape() {
        let status = CellVzStatus::gate_off("Cell VZ not enabled (set LATTICE_CELL_VZ=1)");
        assert!(!status.up);
        assert!(!status.ping_ok);
        assert!(status.error.is_some());
    }

    #[test]
    fn has_vz_artifacts_requires_rootfs_and_kernel() {
        let dir = std::env::temp_dir().join(format!(
            "cell-vz-artifacts-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        assert!(!has_vz_artifacts(&dir));
        std::fs::write(dir.join("cellos.ext4"), b"x").expect("rootfs");
        assert!(!has_vz_artifacts(&dir));
        std::fs::write(dir.join("vmlinux"), b"k").expect("kernel");
        assert!(has_vz_artifacts(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn start_fails_fast_when_host_binary_missing() {
        let config = CellVzConfig {
            host_bin: PathBuf::from("/nonexistent/cell-host-macos-test"),
            celld_bin: PathBuf::from("/nonexistent/celld-test"),
            helper_socket: std::env::temp_dir().join("cell-vz-test.sock"),
            images_dir: default_images_dir(),
            data_dir: std::env::temp_dir().join("cell-vz-data-test"),
            listen: DEFAULT_LISTEN.parse().expect("listen"),
            cell_id: DEFAULT_CELL_ID.to_string(),
            ping_timeout: DEFAULT_PING_TIMEOUT,
        };
        match CellVzController::start(CellVzProviderMode::Supervised(config)).await {
            Err(err) => {
                let message = err.to_string();
                assert!(
                    message.contains("cell-host-macos"),
                    "unexpected error: {message}"
                );
                assert!(
                    message.contains("LATTICE_CELL_HOST_BIN"),
                    "unexpected error: {message}"
                );
            }
            Ok(_) => panic!("expected missing binary error"),
        }
    }

    #[test]
    fn resolve_bin_helpers_do_not_panic() {
        let _ = resolve_cell_host_bin();
        let _ = resolve_celld_bin();
    }
}
