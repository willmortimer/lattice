//! Idle shutdown and keep-running integration tests.

use std::sync::Arc;
use std::time::Duration;

use lattice_client::{
    request, response, DaemonClient, HealthRequest, LatticeClient, OpenWorkspaceRequest, Request,
};
use lattice_core::Workspace;
use lattice_daemon::{lease_path, serve_with_shutdown, spawn_latticed, DaemonConfig, SpawnOptions};
use lattice_runtime::{read_workspace_lease, LatticeRuntime, OWNER_LATTICED};
use tempfile::TempDir;

fn health_request() -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: None,
        body: Some(request::Body::Health(HealthRequest {})),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_shutdown_exits_after_last_client_disconnects() {
    let dir = TempDir::new().expect("tempdir");
    let socket = dir.path().join("idle.sock");
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "idle-token")
        .with_instance_id("idle-exit")
        .with_keep_services_running(false)
        .with_idle_shutdown_secs(1)
        .with_ready_timeout(Duration::from_secs(10));

    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect");
    let _ = client.request(health_request()).await.expect("health");
    drop(client);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(status)) = spawned.try_wait() {
            assert!(status.success(), "daemon should exit 0 on idle shutdown");
            assert!(!spawned.socket_path.exists(), "socket file should be removed");
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    spawned.kill();
    panic!("daemon did not exit after idle timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keep_running_stays_up_after_client_disconnects() {
    let dir = TempDir::new().expect("tempdir");
    let socket = dir.path().join("keep.sock");
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "keep-token")
        .with_instance_id("keep-up")
        .with_keep_services_running(true)
        .with_idle_shutdown_secs(1)
        .with_ready_timeout(Duration::from_secs(10));

    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect");
    let _ = client.request(health_request()).await.expect("health");
    drop(client);

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        spawned.socket_path.exists(),
        "socket should remain while keep-running is enabled"
    );
    assert!(
        spawned.try_wait().expect("try_wait").is_none(),
        "daemon should still be running"
    );

    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("reconnect");
    let health = client.request(health_request()).await.expect("health");
    match health.body {
        Some(response::Body::Health(h)) => assert_eq!(h.instance_id, "keep-up"),
        other => panic!("unexpected health: {other:?}"),
    }
    spawned.kill();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_releases_workspace_lease() {
    let fixture = TempDir::new().expect("fixture");
    Workspace::init(fixture.path(), "Idle Lease").expect("init workspace");
    let workspace_path = fixture.path().to_string_lossy().into_owned();

    let dir = TempDir::new().expect("tempdir");
    let socket = dir.path().join("lease.sock");
    let auth_token = "lease-token".to_string();
    let config = DaemonConfig::new(&socket, auth_token.clone())
        .with_instance_id("lease-release")
        .with_api_port(None)
        .with_keep_services_running(false)
        .with_idle_shutdown_timeout(Duration::from_millis(100));
    let runtime = Arc::new(LatticeRuntime::new());
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let serve = tokio::spawn(serve_with_shutdown(
        config,
        Arc::clone(&runtime),
        shutdown_rx,
    ));

    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let client = DaemonClient::connect(&socket, &auth_token)
        .await
        .expect("connect");
    let open = client
        .request(Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
                path: workspace_path,
            })),
        })
        .await
        .expect("open");
    match open.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            let lease = resp.lease.expect("lease");
            assert_eq!(lease.owner, OWNER_LATTICED);
        }
        other => panic!("unexpected open: {other:?}"),
    }
    assert!(lease_path(fixture.path()).is_file());
    drop(client);

    let _ = shutdown_tx.send(());
    serve.await.unwrap().unwrap();

    assert!(
        read_workspace_lease(fixture.path())
            .expect("read lease")
            .is_none(),
        "lease file should be cleared on shutdown"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_shutdown_respects_configured_timeout() {
    let dir = TempDir::new().expect("tempdir");
    let socket = dir.path().join("idle-bounded.sock");
    let idle_secs = 1u64;
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "idle-bounded-token")
        .with_instance_id("idle-bounded")
        .with_keep_services_running(false)
        .with_idle_shutdown_secs(idle_secs)
        .with_ready_timeout(Duration::from_secs(10));

    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect");
    let _ = client.request(health_request()).await.expect("health");
    drop(client);

    let disconnect_at = std::time::Instant::now();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(status)) = spawned.try_wait() {
            let elapsed_ms = disconnect_at.elapsed().as_millis();
            assert!(status.success(), "daemon should exit 0 on idle shutdown");
            assert!(
                elapsed_ms >= (idle_secs * 1_000) as u128,
                "idle shutdown too fast: {elapsed_ms}ms"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    spawned.kill();
    panic!("daemon did not exit within idle shutdown window");
}
