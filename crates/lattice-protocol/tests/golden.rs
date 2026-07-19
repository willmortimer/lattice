use lattice_protocol::{
    decode_frame, encode_frame, event_envelope, request_envelope, response_envelope,
    ApplyPageUpdateRequest, ApplyPageUpdateResponse, Event, HealthRequest, HealthResponse,
    OpenWorkspaceRequest, OpenWorkspaceResponse, PingRequest, Request, Response, SearchRequest,
    SearchResponse, WorkspaceLease, WorkspaceLeaseChanged, PROTOCOL_VERSION,
};
use prost::Message;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn load_hex_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    hex::decode(&cleaned).unwrap_or_else(|err| panic!("decode {}: {err}", path.display()))
}

fn health_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-health-req",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::Health(HealthRequest {})),
        },
    )
}

fn health_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-health-res",
        Response {
            body: Some(lattice_protocol::response::Body::Health(HealthResponse {
                status: "ok".into(),
                protocol_version: PROTOCOL_VERSION,
                instance_id: "0190abcdef0123456789".into(),
            })),
        },
    )
}

fn ping_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-ping",
        Request {
            deadline_unix_ms: Some(1_720_000_000_000),
            idempotency_key: Some("idem-golden-ping".into()),
            body: Some(lattice_protocol::request::Body::Ping(PingRequest {
                nonce: "n-42".into(),
            })),
        },
    )
}

fn open_workspace_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-open",
        Request {
            deadline_unix_ms: None,
            idempotency_key: Some("idem-open-ws".into()),
            body: Some(lattice_protocol::request::Body::OpenWorkspace(
                OpenWorkspaceRequest {
                    path: "/tmp/example-workspace".into(),
                },
            )),
        },
    )
}

fn open_workspace_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-open",
        Response {
            body: Some(lattice_protocol::response::Body::OpenWorkspace(
                OpenWorkspaceResponse {
                    workspace_id: "ws-1".into(),
                    lease: Some(WorkspaceLease {
                        schema_version: 1,
                        owner: "latticed".into(),
                        pid: 12345,
                        process_start: 987_654_321,
                        socket: "/tmp/latticed.sock".into(),
                        protocol_version: PROTOCOL_VERSION,
                        instance_id: "0190instance".into(),
                        acquired_at: "2026-07-19T20:00:00Z".into(),
                    }),
                },
            )),
        },
    )
}

fn search_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-search",
        Request {
            deadline_unix_ms: None,
            idempotency_key: None,
            body: Some(lattice_protocol::request::Body::Search(SearchRequest {
                workspace_id: "ws-1".into(),
                query: "hello lattice".into(),
            })),
        },
    )
}

fn search_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-search",
        Response {
            body: Some(lattice_protocol::response::Body::Search(SearchResponse {})),
        },
    )
}

fn apply_page_update_request() -> lattice_protocol::Envelope {
    request_envelope(
        "golden-apply",
        Request {
            deadline_unix_ms: None,
            idempotency_key: Some("idem-apply-1".into()),
            body: Some(lattice_protocol::request::Body::ApplyPageUpdate(
                ApplyPageUpdateRequest {
                    workspace_id: "ws-1".into(),
                    path: "Notes.md".into(),
                    content: "# Updated\n".into(),
                    expected_revision: "sha256:abc".into(),
                },
            )),
        },
    )
}

fn apply_page_update_response() -> lattice_protocol::Envelope {
    response_envelope(
        "golden-apply",
        Response {
            body: Some(lattice_protocol::response::Body::ApplyPageUpdate(
                ApplyPageUpdateResponse {
                    revision: "sha256:def".into(),
                },
            )),
        },
    )
}

fn lease_changed_event() -> lattice_protocol::Envelope {
    event_envelope(
        "",
        Event {
            sequence: 7,
            workspace_id: "ws-1".into(),
            body: Some(lattice_protocol::event::Body::LeaseChanged(
                WorkspaceLeaseChanged {
                    lease: Some(WorkspaceLease {
                        schema_version: 1,
                        owner: "embedded".into(),
                        pid: 99,
                        process_start: 1,
                        socket: String::new(),
                        protocol_version: PROTOCOL_VERSION,
                        instance_id: "embedded-1".into(),
                        acquired_at: "2026-07-19T21:00:00Z".into(),
                    }),
                },
            )),
        },
    )
}

fn assert_golden(name: &str, envelope: &lattice_protocol::Envelope) {
    let expected = load_hex_fixture(name);
    let actual = envelope.encode_to_vec();
    assert_eq!(
        hex::encode(&actual),
        hex::encode(&expected),
        "protobuf bytes for {name} drifted from golden fixture"
    );
    let decoded = lattice_protocol::Envelope::decode(expected.as_slice()).expect("decode golden");
    assert_eq!(&decoded, envelope);

    let framed = encode_frame(envelope).expect("frame encode");
    let round_trip = decode_frame(&framed).expect("frame decode");
    assert_eq!(&round_trip, envelope);
}

#[test]
fn golden_health_request() {
    assert_golden("health_request.hex", &health_request());
}

#[test]
fn golden_health_response() {
    assert_golden("health_response.hex", &health_response());
}

#[test]
fn golden_ping_request() {
    assert_golden("ping_request.hex", &ping_request());
}

#[test]
fn golden_open_workspace_request() {
    assert_golden("open_workspace_request.hex", &open_workspace_request());
}

#[test]
fn golden_open_workspace_response() {
    assert_golden("open_workspace_response.hex", &open_workspace_response());
}

#[test]
fn golden_search_request() {
    assert_golden("search_request.hex", &search_request());
}

#[test]
fn golden_search_response() {
    assert_golden("search_response.hex", &search_response());
}

#[test]
fn golden_apply_page_update_request() {
    assert_golden(
        "apply_page_update_request.hex",
        &apply_page_update_request(),
    );
}

#[test]
fn golden_apply_page_update_response() {
    assert_golden(
        "apply_page_update_response.hex",
        &apply_page_update_response(),
    );
}

#[test]
fn golden_lease_changed_event() {
    assert_golden("lease_changed_event.hex", &lease_changed_event());
}

#[test]
fn protocol_version_constant_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(health_response().protocol_version, 1);
}
