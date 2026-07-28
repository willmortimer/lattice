//! KernelFS WASI guest → Lattice proposal drafts → mocked propose_resource.

use std::fs;
use std::sync::Arc;

use kernelfs::{ExecutionManifest, InputMount, Mounts, WasmtimeLimits};
use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::wasi_host::{propose_output_drafts, run_wasi_guest, WorkspaceBinding};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn copy_hello_wasm() -> &'static [u8] {
    include_bytes!("../../kernelfs/fixtures/copy_hello.wasm")
}

struct ProposeResourceCapture {
    bodies: Arc<std::sync::Mutex<Vec<Value>>>,
}

impl Respond for ProposeResourceCapture {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if let Ok(body) = serde_json::from_slice::<Value>(&request.body) {
            if let Ok(mut guard) = self.bodies.lock() {
                guard.push(body);
            }
        }
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_json(json!({
                "proposalId": "prop_test",
                "status": "open"
            }))
    }
}

#[tokio::test]
async fn wasi_guest_output_proposes_via_latticed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "hello from input").expect("write hello");

    let manifest = ExecutionManifest {
        run_id: "run_agentd_wasi".into(),
        base_snapshot: "snap_1".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: host_input.join("hello.txt"),
                guest_path: "hello.txt".into(),
            }],
            output_proposal_target: Some("Reports".into()),
            work_promote_paths: Vec::new(),
        },
        capabilities: Default::default(),
    };

    let run_parent = temp.path().to_path_buf();
    let drafts = tokio::task::spawn_blocking(move || {
        run_wasi_guest(
            &run_parent,
            &manifest,
            copy_hello_wasm(),
            &WasmtimeLimits::default(),
        )
    })
    .await
    .expect("join wasi")
    .expect("run wasi guest");

    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].resource_path, "Reports/out.txt");
    assert_eq!(drafts[0].content, b"hello from input");

    let latticed = MockServer::start().await;
    let bodies = Arc::new(std::sync::Mutex::new(Vec::new()));
    let capture = ProposeResourceCapture {
        bodies: Arc::clone(&bodies),
    };

    Mock::given(method("POST"))
        .and(path("/v1/proposals/propose_resource"))
        .respond_with(capture)
        .expect(1)
        .mount(&latticed)
        .await;

    let client = LatticeToolClient::new(latticed.uri(), "test-token").expect("client");
    let responses = propose_output_drafts(
        &client,
        &WorkspaceBinding::new(Some("ws-1".into()), None),
        &drafts,
    )
    .await
    .expect("propose drafts");

    assert_eq!(responses.len(), 1);
    assert_eq!(
        responses[0].get("proposalId").and_then(|v| v.as_str()),
        Some("prop_test")
    );

    let captured = bodies.lock().expect("lock bodies");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].get("workspaceId").and_then(|v| v.as_str()),
        Some("ws-1")
    );
    assert_eq!(
        captured[0].get("path").and_then(|v| v.as_str()),
        Some("Reports/out.txt")
    );
    assert_eq!(
        captured[0].get("content").and_then(|v| v.as_str()),
        Some("hello from input")
    );
    assert!(captured[0]
        .get("summary")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.contains("Reports/out.txt")));
}
