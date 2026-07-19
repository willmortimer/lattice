//! On-demand `latticed` process launch helpers (for desktop later).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use lattice_client::DaemonClient;
use tokio::time::{sleep, Instant};

use crate::error::{Error, Result};
use crate::preferences::DaemonPreferences;

/// Options for spawning a `latticed` child process.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// Path to the `latticed` binary.
    pub binary: PathBuf,
    /// Socket path the child should bind.
    pub socket_path: PathBuf,
    /// Auth token the child should require.
    pub auth_token: String,
    /// Optional fixed instance id (otherwise the child generates one).
    pub instance_id: Option<String>,
    /// How long to wait for the socket / handshake to become ready.
    pub ready_timeout: Duration,
    /// Override profile keep-running preference (`None` reads desktop settings).
    pub keep_services_running: Option<bool>,
    /// Override idle shutdown seconds when keep-running is false.
    pub idle_shutdown_secs: Option<u64>,
}

impl SpawnOptions {
    pub fn new(
        binary: impl Into<PathBuf>,
        socket_path: impl Into<PathBuf>,
        auth_token: impl Into<String>,
    ) -> Self {
        Self {
            binary: binary.into(),
            socket_path: socket_path.into(),
            auth_token: auth_token.into(),
            instance_id: None,
            ready_timeout: Duration::from_secs(5),
            keep_services_running: None,
            idle_shutdown_secs: None,
        }
    }

    pub fn with_instance_id(mut self, instance_id: impl Into<String>) -> Self {
        self.instance_id = Some(instance_id.into());
        self
    }

    pub fn with_ready_timeout(mut self, timeout: Duration) -> Self {
        self.ready_timeout = timeout;
        self
    }

    /// Override keep-running lifecycle behavior for the spawned child.
    pub fn with_keep_services_running(mut self, keep_services_running: bool) -> Self {
        self.keep_services_running = Some(keep_services_running);
        self
    }

    /// Override idle shutdown seconds for the spawned child.
    pub fn with_idle_shutdown_secs(mut self, idle_shutdown_secs: u64) -> Self {
        self.idle_shutdown_secs = Some(idle_shutdown_secs);
        self
    }
}

/// Handle for a spawned `latticed` child.
pub struct SpawnedDaemon {
    child: Child,
    pub socket_path: PathBuf,
    pub auth_token: String,
    pub instance_id: String,
}

impl SpawnedDaemon {
    /// Child process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Attempt a graceful kill; ignores errors if already exited.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Non-blocking check whether the child has exited.
    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }
}

impl Drop for SpawnedDaemon {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Spawn `latticed`, wait until a client can connect and health-check.
pub async fn spawn_latticed(opts: SpawnOptions) -> Result<SpawnedDaemon> {
    if let Some(parent) = opts.socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if opts.socket_path.exists() {
        std::fs::remove_file(&opts.socket_path)?;
    }

    let prefs = DaemonPreferences::load();
    let keep_services_running = opts
        .keep_services_running
        .unwrap_or(prefs.keep_services_running);
    let idle_shutdown_secs = opts.idle_shutdown_secs.unwrap_or_else(|| {
        prefs.idle_shutdown_timeout.as_secs().max(1)
    });

    let mut cmd = Command::new(&opts.binary);
    cmd.arg("--socket")
        .arg(&opts.socket_path)
        .arg("--auth-token")
        .arg(&opts.auth_token)
        // Spawned helpers are for the Unix socket control plane; disable the
        // localhost HTTP API unless a caller starts latticed directly.
        .arg("--api-port")
        .arg("0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if keep_services_running {
        cmd.arg("--keep-services-running");
    } else {
        cmd.arg("--idle-shutdown-secs").arg(idle_shutdown_secs.to_string());
    }
    if let Some(instance_id) = &opts.instance_id {
        cmd.arg("--instance-id").arg(instance_id);
    }

    let child = cmd
        .spawn()
        .map_err(|err| Error::Spawn(format!("failed to spawn {}: {err}", opts.binary.display())))?;

    match wait_for_ready(&opts.socket_path, &opts.auth_token, opts.ready_timeout).await {
        Ok(instance_id) => Ok(SpawnedDaemon {
            child,
            socket_path: opts.socket_path,
            auth_token: opts.auth_token,
            instance_id,
        }),
        Err(err) => {
            let mut failed = SpawnedDaemon {
                child,
                socket_path: opts.socket_path,
                auth_token: opts.auth_token,
                instance_id: String::new(),
            };
            failed.kill();
            Err(err)
        }
    }
}

/// Poll until `DaemonClient` can connect and complete a Health request.
pub async fn wait_for_ready(
    socket_path: impl AsRef<Path>,
    auth_token: &str,
    timeout: Duration,
) -> Result<String> {
    let socket_path = socket_path.as_ref();
    let deadline = Instant::now() + timeout;
    let mut last_err = None;

    while Instant::now() < deadline {
        if socket_path.exists() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match tokio::time::timeout(remaining, try_health(socket_path, auth_token)).await {
                Ok(Ok(instance_id)) => return Ok(instance_id),
                Ok(Err(err)) => last_err = Some(err),
                Err(_) => {
                    return Err(Error::ReadyTimeout {
                        path: format!(
                            "{} ({})",
                            socket_path.display(),
                            last_err.unwrap_or_else(|| "health check timed out".into())
                        ),
                    });
                }
            }
        }
        sleep(Duration::from_millis(25)).await;
    }

    Err(Error::ReadyTimeout {
        path: format!(
            "{} ({})",
            socket_path.display(),
            last_err.unwrap_or_else(|| "socket never became ready".into())
        ),
    })
}

async fn try_health(socket_path: &Path, auth_token: &str) -> std::result::Result<String, String> {
    use lattice_client::{request, HealthRequest, LatticeClient, Request};
    let client = DaemonClient::connect(socket_path, auth_token)
        .await
        .map_err(|err| err.to_string())?;
    client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::Health(HealthRequest {})),
        })
        .await
        .map_err(|err| err.to_string())?;
    Ok(client.instance_id().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DaemonConfig;
    use crate::server::serve_with_shutdown;
    use lattice_runtime::LatticeRuntime;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::oneshot;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_ready_against_in_process_server() {
        let dir = tempdir().unwrap();
        let socket = dir.path().join("latticed.sock");
        let config = DaemonConfig::new(&socket, "ready-tok")
            .with_instance_id("ready-id")
            .with_api_port(None);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let runtime = Arc::new(LatticeRuntime::new());
        let serve = tokio::spawn(serve_with_shutdown(config, runtime, shutdown_rx));

        let instance_id = wait_for_ready(&socket, "ready-tok", Duration::from_secs(2))
            .await
            .expect("ready");
        assert_eq!(instance_id, "ready-id");

        let _ = shutdown_tx.send(());
        serve.await.unwrap().unwrap();
    }
}
