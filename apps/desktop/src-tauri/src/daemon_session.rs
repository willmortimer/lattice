//! Shared latticed connect/spawn for desktop thin clients (voice, semantic).
//!
//! Voice and semantic each keep their own state and optional child handle.
//! The first feature to spawn owns the child; a later feature connects to the
//! existing socket using `LATTICE_AUTH_TOKEN` (set in-process when spawning).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use lattice_client::DaemonClient;

pub const ENV_SOCKET: &str = "LATTICE_SOCKET";
pub const ENV_AUTH_TOKEN: &str = "LATTICE_AUTH_TOKEN";
pub const ENV_LATTICED_BIN: &str = "LATTICE_LATTICED_BIN";

/// Extra environment applied only when this process spawns latticed.
pub struct SpawnHostEnv {
    pub extra_env: Vec<(String, String)>,
    /// Appended to handshake-failure messages after a spawn.
    pub handshake_hint: Option<&'static str>,
}

impl Default for SpawnHostEnv {
    fn default() -> Self {
        Self {
            extra_env: Vec::new(),
            handshake_hint: None,
        }
    }
}

/// Keeps a desktop-spawned daemon alive; Drop kills the child.
pub struct SpawnedDaemon {
    child: Child,
}

impl Drop for SpawnedDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn default_socket_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join("run")
        .join("latticed.sock")
}

pub fn socket_path() -> PathBuf {
    std::env::var_os(ENV_SOCKET)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_socket_path)
}

pub fn which_bin(name: &str) -> std::io::Result<PathBuf> {
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

/// Sibling of the running executable (e.g. `Lattice.app/Contents/MacOS/latticed`).
pub fn current_exe_sibling(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}

pub fn resolve_latticed_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_LATTICED_BIN) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(path) = which_bin("latticed") {
        return Some(path);
    }
    // Finder-launched .app: helpers live beside lattice-desktop in Contents/MacOS.
    if let Some(path) = current_exe_sibling("latticed") {
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

pub fn wait_for_socket(socket: &Path, timeout: Duration) -> Result<(), String> {
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

fn spawn_latticed(
    binary: &Path,
    socket: &Path,
    auth_token: &str,
    host_env: &SpawnHostEnv,
) -> Result<Child, String> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if socket.exists() {
        let _ = std::fs::remove_file(socket);
    }
    let mut command = Command::new(binary);
    command
        .arg("--socket")
        .arg(socket)
        .arg("--auth-token")
        .arg(auth_token)
        .arg("--api-port")
        .arg("0")
        .arg("--keep-services-running");

    for (key, value) in &host_env.extra_env {
        command.env(key, value);
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to spawn {}: {err}", binary.display()))
}

/// Connect to an existing latticed, or spawn one with `host_env.extra_env`.
///
/// When spawning, always `set_var(LATTICE_AUTH_TOKEN, …)` so other desktop
/// features in this process can attach to the same daemon.
pub async fn connect_or_spawn(
    host_env: SpawnHostEnv,
) -> Result<(Arc<DaemonClient>, Option<SpawnedDaemon>), String> {
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
    // Share the token with other desktop modules in this process.
    std::env::set_var(ENV_AUTH_TOKEN, &token);
    let child = spawn_latticed(&binary, &socket, &token, &host_env)?;
    wait_for_socket(&socket, Duration::from_secs(8))?;
    let client = DaemonClient::connect(&socket, &token)
        .await
        .map_err(|err| {
            let hint = host_env
                .handshake_hint
                .unwrap_or("ensure host services for this feature are available");
            format!(
                "spawned latticed at {} but handshake failed: {err} ({hint})",
                socket.display()
            )
        })?;
    Ok((Arc::new(client), Some(SpawnedDaemon { child })))
}
