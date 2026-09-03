//! Shared latticed connect/spawn for desktop thin clients (voice, semantic).
//!
//! Voice and semantic each keep their own state and optional child handle.
//! The first feature to spawn owns the child; a later feature connects to the
//! existing socket using `LATTICE_AUTH_TOKEN` (set in-process when spawning,
//! and persisted beside the socket so relaunches can reattach).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use lattice_client::{default_endpoint, DaemonClient};

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
    default_endpoint()
}

pub fn socket_path() -> PathBuf {
    std::env::var_os(ENV_SOCKET)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_socket_path)
}

/// Persisted auth token path for the daemon endpoint.
///
/// Unix: beside the UDS (`latticed.sock` → `latticed.token`).
/// Windows: `{data}/Lattice/run/latticed.token` (named pipes have no filesystem sibling).
pub fn auth_token_path(socket: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        socket.with_file_name("latticed.token")
    }
    #[cfg(windows)]
    {
        let _ = socket;
        default_run_dir().join("latticed.token")
    }
}

fn default_run_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join("run")
}

pub fn which_bin(name: &str) -> std::io::Result<PathBuf> {
    let path = std::env::var_os("PATH")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "PATH not set"))?;
    let candidates = binary_name_candidates(name);
    for dir in std::env::split_paths(&path) {
        for candidate_name in &candidates {
            let candidate = dir.join(candidate_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
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
    binary_name_candidates(name)
        .into_iter()
        .map(|candidate| dir.join(candidate))
        .find(|path| path.is_file())
}

fn binary_name_candidates(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        if name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("exe"))
        {
            vec![name.to_string()]
        } else {
            vec![format!("{name}.exe"), name.to_string()]
        }
    }
    #[cfg(not(windows))]
    {
        vec![name.to_string()]
    }
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
    // Packaged desktop: helpers live beside the main exe (NSIS/macOS Contents/MacOS).
    if let Some(path) = current_exe_sibling("latticed") {
        return Some(path);
    }
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/latticed"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/latticed"),
        PathBuf::from("target/debug/latticed"),
        PathBuf::from("target/release/latticed"),
    ];
    candidates
        .into_iter()
        .flat_map(|path| {
            binary_name_candidates(
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("latticed"),
            )
            .into_iter()
            .map(move |name| path.with_file_name(name))
        })
        .find(|p| p.is_file())
}

/// Wait until the daemon endpoint accepts connections (not filesystem presence on Windows).
pub async fn wait_for_socket(socket: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if endpoint_accepts_connections(socket).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(format!(
        "timed out waiting for latticed endpoint {}",
        socket.display()
    ))
}

fn read_persisted_auth_token(socket: &Path) -> Option<String> {
    let path = auth_token_path(socket);
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn write_persisted_auth_token(socket: &Path, token: &str) -> Result<(), String> {
    let path = auth_token_path(socket);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
        file.write_all(token.as_bytes())
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, token)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }
    Ok(())
}

/// True when a process appears to be accepting connections on `endpoint`.
async fn endpoint_accepts_connections(endpoint: &Path) -> bool {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(endpoint).await.is_ok()
    }
    #[cfg(windows)]
    {
        windows_endpoint_accepts_connections(endpoint)
    }
}

#[cfg(windows)]
fn windows_endpoint_accepts_connections(endpoint: &Path) -> bool {
    use tokio::net::windows::named_pipe::ClientOptions;

    // ERROR_PIPE_BUSY — server exists but has no free instance yet.
    const ERROR_PIPE_BUSY: i32 = 231;

    match ClientOptions::new().open(endpoint) {
        Ok(_client) => true,
        Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) => true,
        Err(_) => false,
    }
}

/// Unix: stale socket file left on disk. Windows named pipes are never filesystem paths.
fn endpoint_file_present(endpoint: &Path) -> bool {
    #[cfg(unix)]
    {
        endpoint.exists()
    }
    #[cfg(windows)]
    {
        let _ = endpoint;
        false
    }
}

fn clear_stale_socket(socket: &Path) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(socket);
    }
    let _ = std::fs::remove_file(auth_token_path(socket));
}

fn resolve_auth_token(socket: &Path) -> Option<String> {
    std::env::var(ENV_AUTH_TOKEN)
        .ok()
        .filter(|t| !t.is_empty())
        .or_else(|| read_persisted_auth_token(socket))
}

fn install_process_auth_token(token: &str) {
    std::env::set_var(ENV_AUTH_TOKEN, token);
}

fn spawn_latticed(
    binary: &Path,
    socket: &Path,
    auth_token: &str,
    host_env: &SpawnHostEnv,
) -> Result<Child, String> {
    #[cfg(unix)]
    {
        if let Some(parent) = socket.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        if socket.exists() {
            let _ = std::fs::remove_file(socket);
        }
    }
    let mut command = Command::new(binary);
    command
        .arg("--socket")
        .arg(socket)
        .arg("--auth-token")
        .arg(auth_token)
        // Localhost HTTP API (127.0.0.1) for agentd Lattice tools / MCP parity.
        .arg("--api-port")
        .arg("18787")
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
/// When spawning, always `set_var(LATTICE_AUTH_TOKEN, …)` and persist the token
/// beside the socket so other desktop modules / relaunches can attach.
pub async fn connect_or_spawn(
    host_env: SpawnHostEnv,
) -> Result<(Arc<DaemonClient>, Option<SpawnedDaemon>), String> {
    let socket = socket_path();
    let mut token = resolve_auth_token(&socket);
    let accepts = endpoint_accepts_connections(&socket).await;
    let file_present = endpoint_file_present(&socket);

    if accepts || file_present {
        if let Some(existing_token) = token.clone() {
            match DaemonClient::connect(&socket, &existing_token).await {
                Ok(client) => {
                    install_process_auth_token(&existing_token);
                    let _ = write_persisted_auth_token(&socket, &existing_token);
                    return Ok((Arc::new(client), None));
                }
                Err(_) if !accepts => {
                    // Dead socket left behind after a crash / kill.
                    clear_stale_socket(&socket);
                    token = None;
                }
                Err(err) => {
                    return Err(format!(
                        "connect to latticed at {}: {err}",
                        socket.display()
                    ));
                }
            }
        } else if accepts {
            return Err(format!(
                "latticed is running at {} but no auth token is available \
                 (missing {ENV_AUTH_TOKEN} and {}); quit the other Lattice/latticed \
                 instance or remove the socket to recover",
                socket.display(),
                auth_token_path(&socket).display()
            ));
        } else {
            clear_stale_socket(&socket);
        }
    }

    let binary = resolve_latticed_bin().ok_or_else(|| {
        format!(
            "latticed not running at {} and no binary found \
             (set {ENV_LATTICED_BIN} or build `latticed`)",
            socket.display()
        )
    })?;
    let token = token.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    install_process_auth_token(&token);
    write_persisted_auth_token(&socket, &token)?;
    let child = spawn_latticed(&binary, &socket, &token, &host_env)?;
    wait_for_socket(&socket, Duration::from_secs(8)).await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_token_path_sits_beside_socket() {
        let socket = PathBuf::from("/tmp/Lattice/run/latticed.sock");
        assert_eq!(
            auth_token_path(&socket),
            PathBuf::from("/tmp/Lattice/run/latticed.token")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_binary_candidates_prefer_exe_suffix() {
        assert_eq!(
            binary_name_candidates("latticed"),
            vec!["latticed.exe".to_string(), "latticed".to_string()]
        );
        assert_eq!(
            binary_name_candidates("latticed.exe"),
            vec!["latticed.exe".to_string()]
        );
    }

    #[cfg(windows)]
    #[test]
    fn auth_token_path_uses_run_dir_on_windows() {
        let socket = PathBuf::from(r"\\.\pipe\lattice-latticed-alice");
        let path = auth_token_path(&socket);
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("latticed.token")
        );
        assert!(
            path.to_string_lossy().contains("Lattice"),
            "expected Lattice run dir, got {}",
            path.display()
        );
    }

    #[test]
    fn persisted_token_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("latticed.sock");
        write_persisted_auth_token(&socket, "secret-token").unwrap();
        assert_eq!(
            read_persisted_auth_token(&socket).as_deref(),
            Some("secret-token")
        );
    }

    #[test]
    fn default_socket_path_matches_client_default() {
        assert_eq!(default_socket_path(), default_endpoint());
    }
}
