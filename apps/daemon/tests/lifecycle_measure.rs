//! Cold-start and keep-running lifecycle timing (machine-collectable via `LIFECYCLE_MEAS:`).

use std::time::{Duration, Instant};

use lattice_client::{
    request, response, DaemonClient, HealthRequest, LatticeClient, Request,
};
use lattice_daemon::{spawn_latticed, SpawnOptions};

fn health_request() -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: None,
        body: Some(request::Body::Health(HealthRequest {})),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn measure_latticed_cold_start_ready() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let socket = dir.path().join("cold.sock");
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "cold-token")
        .with_instance_id("cold-measure")
        .with_keep_services_running(true)
        .with_ready_timeout(Duration::from_secs(10));

    let start = Instant::now();
    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    let ready_ms = start.elapsed().as_millis();
    eprintln!("LIFECYCLE_MEAS: latticed_cold_start_ready_ms={ready_ms}");

    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect");
    let health = client.request(health_request()).await.expect("health");
    match health.body {
        Some(response::Body::Health(h)) => assert_eq!(h.instance_id, "cold-measure"),
        other => panic!("unexpected health: {other:?}"),
    }
    spawned.kill();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn measure_idle_shutdown_after_disconnect() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let socket = dir.path().join("idle-timing.sock");
    let idle_secs = 1u64;
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "idle-timing-token")
        .with_instance_id("idle-timing")
        .with_keep_services_running(false)
        .with_idle_shutdown_secs(idle_secs)
        .with_ready_timeout(Duration::from_secs(10));

    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect");
    let _ = client.request(health_request()).await.expect("health");
    drop(client);

    let disconnect_at = Instant::now();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(status)) = spawned.try_wait() {
            let shutdown_ms = disconnect_at.elapsed().as_millis();
            eprintln!("LIFECYCLE_MEAS: idle_shutdown_after_disconnect_ms={shutdown_ms}");
            assert!(status.success(), "daemon should exit 0 on idle shutdown");
            assert!(
                shutdown_ms >= (idle_secs * 1_000) as u128,
                "shutdown should respect idle timeout (got {shutdown_ms}ms)"
            );
            assert!(
                shutdown_ms < 4_000,
                "idle shutdown should complete within a few seconds (got {shutdown_ms}ms)"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    spawned.kill();
    panic!("daemon did not exit after idle timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn measure_keep_running_survives_disconnect() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let socket = dir.path().join("keep-timing.sock");
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "keep-timing-token")
        .with_instance_id("keep-timing")
        .with_keep_services_running(true)
        .with_idle_shutdown_secs(1)
        .with_ready_timeout(Duration::from_secs(10));

    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect");
    let _ = client.request(health_request()).await.expect("health");
    drop(client);

    let wait_start = Instant::now();
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let wait_ms = wait_start.elapsed().as_millis();
    eprintln!("LIFECYCLE_MEAS: keep_running_wait_after_disconnect_ms={wait_ms}");

    assert!(
        spawned.socket_path.exists(),
        "socket should remain while keep-running is enabled"
    );
    assert!(
        spawned.try_wait().expect("try_wait").is_none(),
        "daemon should still be running"
    );

    let reconnect_start = Instant::now();
    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("reconnect");
    let reconnect_ms = reconnect_start.elapsed().as_millis();
    eprintln!("LIFECYCLE_MEAS: keep_running_reconnect_ms={reconnect_ms}");
    let health = client.request(health_request()).await.expect("health");
    match health.body {
        Some(response::Body::Health(h)) => assert_eq!(h.instance_id, "keep-timing"),
        other => panic!("unexpected health: {other:?}"),
    }
    spawned.kill();
}
