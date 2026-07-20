//! Voice-host crash isolation: daemon survives supervised host death and recovers.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lattice_client::{
    request, response, DaemonClient, LatticeClient, OpenWorkspaceRequest, Request,
};
use lattice_core::Workspace;
use lattice_daemon::{
    resolve_voice_host_bin, serve_with_shutdown_and_controllers, DaemonConfig, VoiceController,
    VoiceProviderMode,
};
use lattice_protocol::GetVoiceCapabilitiesRequest;
use lattice_runtime::LatticeRuntime;
use tempfile::TempDir;
use tokio::sync::oneshot;

struct ServerGuard {
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<lattice_daemon::Result<()>>>,
    _dir: TempDir,
    socket_path: PathBuf,
    auth_token: String,
    voice: Arc<VoiceController>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.voice.shutdown();
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

fn ensure_voice_host_bin() -> PathBuf {
    if let Some(path) = resolve_voice_host_bin() {
        return path;
    }
    let status = std::process::Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "lattice-voice-host",
            "--bin",
            "lattice-voice-host",
        ])
        .status()
        .expect("spawn cargo build lattice-voice-host");
    assert!(
        status.success(),
        "cargo build -p lattice-voice-host failed: {status}"
    );
    resolve_voice_host_bin().expect(
        "lattice-voice-host binary missing after build (set LATTICE_VOICE_HOST_BIN)",
    )
}

async fn spawn_daemon_with_voice() -> ServerGuard {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("latticed.sock");
    let voice_socket = dir.path().join("voice-host.sock");
    let auth_token = "crash-isolation-token".to_string();
    let config = DaemonConfig::new(&socket_path, auth_token.clone())
        .with_instance_id("crash-isolation")
        .with_api_port(None)
        .with_keep_services_running(true);

    let binary = ensure_voice_host_bin();
    let voice = VoiceController::start(VoiceProviderMode::SpawnHost {
        binary,
        socket: voice_socket,
        fake: true,
    })
    .await
    .expect("start voice controller");

    let runtime = Arc::new(LatticeRuntime::new());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join = tokio::spawn(serve_with_shutdown_and_controllers(
        config,
        runtime,
        None,
        Some(Arc::clone(&voice)),
        shutdown_rx,
    ));

    for _ in 0..100 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(socket_path.exists(), "daemon socket should appear");

    ServerGuard {
        shutdown: Some(shutdown_tx),
        join: Some(join),
        _dir: dir,
        socket_path,
        auth_token,
        voice,
    }
}

fn health_request() -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: None,
        body: Some(request::Body::Health(lattice_client::HealthRequest {})),
    }
}

async fn voice_capabilities(client: &DaemonClient) -> Result<(), lattice_client::ClientError> {
    client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::GetVoiceCapabilities(
                GetVoiceCapabilitiesRequest {},
            )),
        })
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervised_voice_host_crash_daemon_survives_and_recovers() {
    let fixture = TempDir::new().expect("fixture");
    Workspace::init(fixture.path(), "Crash Isolation").expect("init workspace");
    let workspace_path = fixture.path().to_string_lossy().into_owned();

    let guard = spawn_daemon_with_voice().await;
    let client = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");

    voice_capabilities(&client)
        .await
        .expect("voice should work before crash");

    let open = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                path: workspace_path.clone(),
            })),
        })
        .await
        .expect("open workspace");
    match open.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            assert!(resp.lease.is_some(), "workspace lease should be granted");
        }
        other => panic!("unexpected open: {other:?}"),
    }

    assert!(
        guard.voice.kill_supervised_host_for_test(),
        "expected supervised fake voice-host"
    );

    let degraded_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < degraded_deadline {
        if guard.voice.is_degraded() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        guard.voice.is_degraded(),
        "voice plane should be degraded after host kill"
    );

    let health = client.request(health_request()).await.expect("daemon health");
    match health.body {
        Some(response::Body::Health(h)) => assert_eq!(h.instance_id, "crash-isolation"),
        other => panic!("unexpected health during degradation: {other:?}"),
    }

    let voice_err = voice_capabilities(&client).await.expect_err("voice during outage");
    let message = voice_err.to_string();
    assert!(
        message.contains("voice_host_unavailable")
            || message.contains("unavailable")
            || message.contains("closed")
            || message.contains("Connection"),
        "expected voice outage error, got: {message}"
    );

    let recovery_start = std::time::Instant::now();
    let recovery_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut recovered = false;
    while tokio::time::Instant::now() < recovery_deadline {
        if voice_capabilities(&client).await.is_ok() && !guard.voice.is_degraded() {
            recovered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(recovered, "voice plane should recover after supervisor restart");
    let recovery_ms = recovery_start.elapsed().as_millis();
    eprintln!("LIFECYCLE_MEAS: voice_host_recovery_ms={recovery_ms}");

    let reopen = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                path: workspace_path,
            })),
        })
        .await
        .expect("reopen workspace after recovery");
    match reopen.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            assert!(resp.lease.is_some(), "workspace lease intact after recovery");
        }
        other => panic!("unexpected reopen: {other:?}"),
    }
}
