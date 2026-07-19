//! Contract tests: EmbeddedClient and DaemonClient share Health/Ping semantics.

mod common;

use common::{spawn_fake_daemon, FakeDaemonConfig};
use lattice_client::{
    request, response, ClientError, DaemonClient, EmbeddedClient, EventFilter, HealthRequest,
    LatticeClient, PingRequest, Request, PROTOCOL_VERSION,
};
use std::time::Duration;

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

async fn assert_health(client: &dyn LatticeClient, expected_instance_id: &str) {
    let response = client.request(health_request()).await.expect("health");
    match response.body {
        Some(response::Body::Health(health)) => {
            assert_eq!(health.status, "ok");
            assert_eq!(health.protocol_version, PROTOCOL_VERSION);
            assert_eq!(health.instance_id, expected_instance_id);
        }
        other => panic!("expected health response, got {other:?}"),
    }
}

async fn assert_ping(client: &dyn LatticeClient, nonce: &str) {
    let response = client.request(ping_request(nonce)).await.expect("ping");
    match response.body {
        Some(response::Body::Ping(ping)) => assert_eq!(ping.nonce, nonce),
        other => panic!("expected ping response, got {other:?}"),
    }
}

#[tokio::test]
async fn embedded_health_and_ping() {
    let client = EmbeddedClient::new("instance-embedded");
    assert_health(&client, "instance-embedded").await;
    assert_ping(&client, "nonce-embedded").await;
}

#[tokio::test]
async fn daemon_health_and_ping_via_fake_server() {
    let config = FakeDaemonConfig {
        auth_token: "test-token".into(),
        instance_id: "instance-daemon".into(),
    };
    let (socket_path, _guard) = spawn_fake_daemon(config.clone()).await;

    let client = DaemonClient::connect(&socket_path, config.auth_token)
        .await
        .expect("connect");
    assert_eq!(client.instance_id(), "instance-daemon");
    assert_health(&client, "instance-daemon").await;
    assert_ping(&client, "nonce-daemon").await;
}

#[tokio::test]
async fn embedded_and_daemon_health_semantics_match() {
    let instance_id = "parity-instance";
    let embedded = EmbeddedClient::new(instance_id);

    let config = FakeDaemonConfig {
        auth_token: "parity-token".into(),
        instance_id: instance_id.into(),
    };
    let (socket_path, _guard) = spawn_fake_daemon(config.clone()).await;
    let daemon = DaemonClient::connect(&socket_path, config.auth_token)
        .await
        .expect("connect");

    let embedded_health = embedded.request(health_request()).await.expect("embedded");
    let daemon_health = daemon.request(health_request()).await.expect("daemon");
    assert_eq!(embedded_health, daemon_health);

    let embedded_ping = embedded
        .request(ping_request("same-nonce"))
        .await
        .expect("embedded ping");
    let daemon_ping = daemon
        .request(ping_request("same-nonce"))
        .await
        .expect("daemon ping");
    assert_eq!(embedded_ping, daemon_ping);
}

#[tokio::test]
async fn daemon_rejects_bad_auth_token() {
    let config = FakeDaemonConfig {
        auth_token: "correct-token".into(),
        instance_id: "instance-auth".into(),
    };
    let (socket_path, _guard) = spawn_fake_daemon(config).await;

    match DaemonClient::connect(&socket_path, "wrong-token").await {
        Err(ClientError::HandshakeRejected { message }) => {
            assert!(!message.is_empty(), "rejection should explain failure");
        }
        Ok(_) => panic!("must reject bad auth token"),
        Err(other) => panic!("expected HandshakeRejected, got {other:?}"),
    }
}

#[tokio::test]
async fn embedded_subscribe_returns_empty_stream() {
    let client = EmbeddedClient::new("sub");
    let mut stream = client
        .subscribe(EventFilter::default())
        .await
        .expect("subscribe");
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn daemon_subscribe_yields_filtered_stream() {
    let config = FakeDaemonConfig {
        auth_token: "tok".into(),
        instance_id: "id".into(),
    };
    let (socket_path, _guard) = spawn_fake_daemon(config.clone()).await;
    let client = DaemonClient::connect(&socket_path, config.auth_token)
        .await
        .expect("connect");
    let mut stream = client
        .subscribe(EventFilter {
            workspace_id: Some("ws".into()),
        })
        .await
        .expect("subscribe");
    // Fake daemon does not push events; the stream stays open without closing.
    let next = tokio::time::timeout(Duration::from_millis(50), stream.next()).await;
    assert!(next.is_err(), "no events expected from fake daemon yet");
}
