//! Contract tests: DaemonClient against real latticed vs EmbeddedClient.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lattice_client::{
    request, response, ApplyPageUpdateRequest, DaemonClient, EmbeddedClient, EventFilter,
    HealthRequest, LatticeClient, OpenWorkspaceRequest, PingRequest, Request, SearchRequest,
    SearchResponse, PROTOCOL_VERSION,
};
use lattice_core::Workspace;
use lattice_daemon::{
    lease_path, serve_with_shutdown, spawn_latticed, DaemonConfig, SpawnOptions,
    WorkspaceLeaseFile, OWNER_EMBEDDED, OWNER_LATTICED,
};
use lattice_runtime::{is_process_alive, write_workspace_lease, LatticeRuntime};
use tempfile::TempDir;
use tokio::sync::oneshot;

struct ServerGuard {
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<lattice_daemon::Result<()>>>,
    _dir: TempDir,
    socket_path: PathBuf,
    auth_token: String,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

async fn spawn_in_process_daemon(instance_id: &str) -> (ServerGuard, Arc<LatticeRuntime>) {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("latticed.sock");
    let auth_token = "contract-token".to_string();
    let config = DaemonConfig::new(&socket_path, auth_token.clone())
        .with_instance_id(instance_id)
        .with_process_start(1_234_567)
        .with_api_port(None);
    // Short debounce so watcher contract tests settle quickly.
    let runtime = Arc::new(LatticeRuntime::with_watch_debounce(
        lattice_core::TEST_DEBOUNCE_TIMEOUT,
    ));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join = tokio::spawn(serve_with_shutdown(
        config,
        Arc::clone(&runtime),
        shutdown_rx,
    ));

    // Wait until the socket exists.
    for _ in 0..100 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(socket_path.exists(), "daemon socket should appear");

    (
        ServerGuard {
            shutdown: Some(shutdown_tx),
            join: Some(join),
            _dir: dir,
            socket_path,
            auth_token,
        },
        runtime,
    )
}

fn health_request() -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: None,
        body: Some(request::Body::Health(HealthRequest {})),
    }
}

fn ping_request(nonce: &str) -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: Some("idem-ping".into()),
        body: Some(request::Body::Ping(PingRequest {
            nonce: nonce.into(),
        })),
    }
}

fn open_request(path: &str) -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: None,
        body: Some(request::Body::OpenWorkspace(OpenWorkspaceRequest {
            path: path.into(),
        })),
    }
}

fn search_request(workspace_id: &str, query: &str) -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: None,
        body: Some(request::Body::Search(SearchRequest {
            workspace_id: workspace_id.into(),
            query: query.into(),
        })),
    }
}

fn apply_request(
    workspace_id: &str,
    path: &str,
    content: &str,
    expected_revision: &str,
    idempotency_key: Option<&str>,
) -> Request {
    Request {
        deadline_unix_ms: None,
        idempotency_key: idempotency_key.map(str::to_string),
        body: Some(request::Body::ApplyPageUpdate(ApplyPageUpdateRequest {
            workspace_id: workspace_id.into(),
            path: path.into(),
            content: content.into(),
            expected_revision: expected_revision.into(),
        })),
    }
}

fn init_fixture() -> TempDir {
    let dir = TempDir::new().expect("fixture");
    Workspace::init(dir.path(), "Daemon Contract").expect("init");
    fs::write(
        dir.path().join("Notes.md"),
        "# Notes\n\nContract search target phrase.\n",
    )
    .expect("write");
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_and_ping_over_socket() {
    let (guard, _) = spawn_in_process_daemon("daemon-health").await;
    let client = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");
    assert_eq!(client.instance_id(), "daemon-health");

    let health = client.request(health_request()).await.expect("health");
    match health.body {
        Some(response::Body::Health(h)) => {
            assert_eq!(h.status, "ok");
            assert_eq!(h.protocol_version, PROTOCOL_VERSION);
            assert_eq!(h.instance_id, "daemon-health");
        }
        other => panic!("unexpected health: {other:?}"),
    }

    let ping = client.request(ping_request("nonce-1")).await.expect("ping");
    match ping.body {
        Some(response::Body::Ping(p)) => assert_eq!(p.nonce, "nonce-1"),
        other => panic!("unexpected ping: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_writes_lease_and_search_matches_embedded() {
    let fixture = init_fixture();
    let path = fixture.path().to_string_lossy().into_owned();
    let instance_id = "parity-instance";

    let (guard, daemon_runtime) = spawn_in_process_daemon(instance_id).await;
    let daemon = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("daemon connect");

    let daemon_open = daemon
        .request(open_request(&path))
        .await
        .expect("daemon open");

    let daemon_ws = match daemon_open.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            let lease = resp.lease.expect("daemon lease");
            assert_eq!(lease.owner, OWNER_LATTICED);
            assert_eq!(lease.instance_id, instance_id);
            assert_eq!(lease.protocol_version, PROTOCOL_VERSION);
            assert_eq!(lease.process_start, 1_234_567);
            assert!(!lease.socket.is_empty());
            resp.workspace_id
        }
        other => panic!("unexpected daemon open: {other:?}"),
    };

    let lease_raw = fs::read_to_string(lease_path(fixture.path())).expect("lease file");
    let lease_file: WorkspaceLeaseFile =
        serde_json::from_str(&lease_raw).expect("parse lease json");
    assert_eq!(lease_file.owner, OWNER_LATTICED);
    assert_eq!(lease_file.instance_id, instance_id);
    assert_eq!(lease_file.schema_version, 1);
    assert_eq!(lease_file.process_start, 1_234_567);
    assert!(lease_raw.contains("\"schemaVersion\""));
    assert!(lease_raw.contains("\"processStart\""));

    assert_eq!(daemon_runtime.session_count(), 1);

    // Embedded open on a separate fixture should acquire its own lease and
    // produce matching search response shapes.
    let embedded_fixture = init_fixture();
    let embedded_path = embedded_fixture.path().to_string_lossy().into_owned();
    let embedded_runtime = Arc::new(LatticeRuntime::new());
    let embedded = EmbeddedClient::new(instance_id)
        .with_process_start(99)
        .with_runtime(Arc::clone(&embedded_runtime));
    let embedded_open = embedded
        .request(open_request(&embedded_path))
        .await
        .expect("embedded open");
    let embedded_ws = match embedded_open.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            let lease = resp.lease.expect("embedded lease");
            assert_eq!(lease.owner, OWNER_EMBEDDED);
            assert_eq!(lease.process_start, 99);
            resp.workspace_id
        }
        other => panic!("unexpected embedded open: {other:?}"),
    };

    let daemon_search = daemon
        .request(search_request(&daemon_ws, "Contract"))
        .await
        .expect("daemon search");
    let embedded_search = embedded
        .request(search_request(&embedded_ws, "Contract"))
        .await
        .expect("embedded search");
    assert!(matches!(
        daemon_search.body,
        Some(response::Body::Search(SearchResponse {}))
    ));
    assert_eq!(daemon_search, embedded_search);

    // Warm index: second search must not force another rebuild on the daemon session.
    let session = daemon_runtime
        .get_session_by_id(&daemon_ws)
        .expect("session");
    let rebuilds = session.index_rebuild_count();
    let _ = daemon
        .request(search_request(&daemon_ws, "Contract"))
        .await
        .expect("daemon search again");
    assert_eq!(session.index_rebuild_count(), rebuilds);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_lease_blocks_embedded_open() {
    let fixture = init_fixture();
    let path = fixture.path().to_string_lossy().into_owned();

    let (guard, _) = spawn_in_process_daemon("xor-daemon").await;
    let daemon = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");
    let _ = daemon
        .request(open_request(&path))
        .await
        .expect("daemon open");

    let embedded = EmbeddedClient::new("xor-embedded")
        .with_process_start(3)
        .with_runtime(Arc::new(LatticeRuntime::new()));
    let err = embedded
        .request(open_request(&path))
        .await
        .expect_err("embedded must be blocked");
    match err {
        lattice_client::ClientError::Remote { code, .. } => assert_eq!(code, "lease_held"),
        other => panic!("expected lease_held, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn embedded_lease_blocks_daemon_open() {
    let fixture = init_fixture();
    let path = fixture.path().to_string_lossy().into_owned();

    let embedded = EmbeddedClient::new("emb-holder")
        .with_process_start(4)
        .with_runtime(Arc::new(LatticeRuntime::new()));
    let _ = embedded
        .request(open_request(&path))
        .await
        .expect("embedded open");
    let lease_raw = fs::read_to_string(lease_path(fixture.path())).expect("lease");
    assert!(lease_raw.contains("\"owner\": \"embedded\""));

    let (guard, _) = spawn_in_process_daemon("xor-blocked").await;
    let daemon = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");
    let err = daemon
        .request(open_request(&path))
        .await
        .expect_err("daemon must be blocked");
    match err {
        lattice_client::ClientError::Remote { code, .. } => assert_eq!(code, "lease_held"),
        other => panic!("expected lease_held, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_lease_is_reclaimed_on_open() {
    let fixture = init_fixture();
    let path = fixture.path().to_string_lossy().into_owned();

    let dead_pid = (50_000..60_000)
        .rev()
        .find(|pid| !is_process_alive(*pid))
        .expect("dead pid");
    write_workspace_lease(
        fixture.path(),
        &WorkspaceLeaseFile {
            schema_version: 1,
            owner: OWNER_LATTICED.into(),
            pid: dead_pid,
            process_start: 1,
            socket: "/tmp/gone.sock".into(),
            protocol_version: PROTOCOL_VERSION,
            instance_id: "stale".into(),
            acquired_at: "1970-01-01T00:00:00Z".into(),
        },
    )
    .expect("write stale");

    let (guard, _) = spawn_in_process_daemon("reclaim").await;
    let daemon = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");
    let open = daemon
        .request(open_request(&path))
        .await
        .expect("reclaim open");
    match open.body {
        Some(response::Body::OpenWorkspace(resp)) => {
            let lease = resp.lease.expect("lease");
            assert_eq!(lease.owner, OWNER_LATTICED);
            assert_eq!(lease.instance_id, "reclaim");
            assert_ne!(lease.pid, dead_pid);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_page_update_happy_path_and_idempotent_retry() {
    let fixture = init_fixture();
    let path = fixture.path().to_string_lossy().into_owned();

    let (guard, _) = spawn_in_process_daemon("mutate").await;
    let daemon = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");
    let open = daemon.request(open_request(&path)).await.expect("open");
    let workspace_id = match open.body {
        Some(response::Body::OpenWorkspace(resp)) => resp.workspace_id,
        other => panic!("unexpected open: {other:?}"),
    };

    let before = lattice_handlers::read_page(path.clone(), "Notes.md".into()).unwrap();
    let first = daemon
        .request(apply_request(
            &workspace_id,
            "Notes.md",
            "# Notes\n\nUpdated via daemon.\n",
            &before.revision,
            Some("idem-mutate-1"),
        ))
        .await
        .expect("apply");
    let rev1 = match first.body {
        Some(response::Body::ApplyPageUpdate(r)) => r.revision,
        other => panic!("unexpected apply: {other:?}"),
    };

    let after = lattice_handlers::read_page(path.clone(), "Notes.md".into()).unwrap();
    assert_eq!(after.content, "# Notes\n\nUpdated via daemon.\n");
    assert_eq!(after.revision, rev1);

    let retry = daemon
        .request(apply_request(
            &workspace_id,
            "Notes.md",
            "# Notes\n\nWould double-apply.\n",
            &before.revision, // stale; must not re-apply
            Some("idem-mutate-1"),
        ))
        .await
        .expect("idempotent retry");
    let rev2 = match retry.body {
        Some(response::Body::ApplyPageUpdate(r)) => r.revision,
        other => panic!("unexpected retry: {other:?}"),
    };
    assert_eq!(rev1, rev2);
    let still = lattice_handlers::read_page(path, "Notes.md".into()).unwrap();
    assert_eq!(still.content, "# Notes\n\nUpdated via daemon.\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_bad_auth_token() {
    let (guard, _) = spawn_in_process_daemon("auth-id").await;
    match DaemonClient::connect(&guard.socket_path, "wrong-token").await {
        Err(lattice_client::ClientError::HandshakeRejected { .. }) => {}
        Ok(_) => panic!("must reject bad auth token"),
        Err(other) => panic!("expected HandshakeRejected, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_workspace_watcher_indexes_external_file_and_emits_events() {
    use lattice_protocol::event::Body;

    let fixture = init_fixture();
    let path = fixture.path().to_string_lossy().into_owned();
    let (guard, runtime) = spawn_in_process_daemon("watch-fts").await;
    let client = DaemonClient::connect(&guard.socket_path, &guard.auth_token)
        .await
        .expect("connect");

    let mut events = client
        .subscribe(EventFilter::default())
        .await
        .expect("subscribe");

    let open = client
        .request(open_request(&path))
        .await
        .expect("open workspace");
    let workspace_id = match open.body {
        Some(response::Body::OpenWorkspace(resp)) => resp.workspace_id,
        other => panic!("unexpected open: {other:?}"),
    };

    let session = runtime
        .get_session_by_id(&workspace_id)
        .expect("warm session");
    assert!(session.is_watching(), "write open must start the watcher");

    // Wait for IndexProgress::started (may race with lease_changed).
    let mut saw_started = false;
    let mut sequences = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline && !saw_started {
        match tokio::time::timeout(
            deadline.saturating_duration_since(tokio::time::Instant::now()),
            events.next(),
        )
        .await
        {
            Ok(Some(Ok(evt))) => {
                sequences.push(evt.sequence);
                if matches!(
                    evt.body,
                    Some(Body::IndexProgress(ref p)) if p.phase == "started"
                ) {
                    saw_started = true;
                }
            }
            Ok(Some(Err(err))) => panic!("event stream error: {err}"),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    assert!(saw_started, "expected IndexProgress started after open");

    fs::write(
        fixture.path().join("External.md"),
        "# External\n\ndaemon-watcher-unique-token\n",
    )
    .expect("write external file");

    let mut saw_upsert = false;
    let mut saw_resource = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline && !(saw_upsert && saw_resource) {
        match tokio::time::timeout(
            deadline.saturating_duration_since(tokio::time::Instant::now()),
            events.next(),
        )
        .await
        {
            Ok(Some(Ok(evt))) => {
                sequences.push(evt.sequence);
                match evt.body {
                    Some(Body::IndexProgress(ref p))
                        if p.phase == "upserted" && p.path.as_deref() == Some("External.md") =>
                    {
                        saw_upsert = true;
                    }
                    Some(Body::ResourceChanged(ref c))
                        if c.path == "External.md"
                            && (c.change == "created" || c.change == "modified") =>
                    {
                        saw_resource = true;
                    }
                    _ => {}
                }
            }
            Ok(Some(Err(err))) => panic!("event stream error: {err}"),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    assert!(saw_resource, "expected ResourceChanged for External.md");
    assert!(saw_upsert, "expected IndexProgress upsert for External.md");
    assert!(
        sequences.windows(2).all(|w| w[1] > w[0]),
        "event sequences must be strictly increasing: {sequences:?}"
    );

    // SearchResponse has no hit payload yet; assert via warm session index.
    let hits = session
        .search("daemon-watcher-unique-token", 10)
        .expect("search");
    assert!(
        hits.iter().any(|h| h.path.ends_with("External.md")),
        "incremental FTS should find externally written file"
    );

    // Exercise the daemon Search RPC path as well.
    let search = client
        .request(search_request(&workspace_id, "daemon-watcher-unique-token"))
        .await
        .expect("daemon search");
    assert!(matches!(
        search.body,
        Some(response::Body::Search(SearchResponse {}))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_helper_launches_binary() {
    let dir = TempDir::new().expect("tempdir");
    let socket = dir.path().join("spawned.sock");
    let opts = SpawnOptions::new(env!("CARGO_BIN_EXE_latticed"), &socket, "spawn-token")
        .with_instance_id("spawned-id")
        .with_ready_timeout(Duration::from_secs(10));

    let mut spawned = spawn_latticed(opts).await.expect("spawn latticed");
    assert_eq!(spawned.instance_id, "spawned-id");

    let client = DaemonClient::connect(&spawned.socket_path, &spawned.auth_token)
        .await
        .expect("connect spawned");
    let health = client.request(health_request()).await.expect("health");
    match health.body {
        Some(response::Body::Health(h)) => assert_eq!(h.instance_id, "spawned-id"),
        other => panic!("unexpected: {other:?}"),
    }

    spawned.kill();
}
