//! celld hydrate → run → collect → mocked propose_resource.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use lattice_agentd::cell_host::{output_map_to_drafts, run_cell_task_and_propose, CellProposalProvenance};
use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::tools::{dispatch_tool, openai_tool_definitions, ToolRunContext};
use lattice_agentd::wasi_host::WorkspaceBinding;
use lattice_cell_client::connect::{encode_connect_message, CELL_APPLY, CELL_START, GUEST_INVOKE};
use lattice_cell_client::{
    celld_configured, CelldClient, CelldHttpClient, HydrateFile, KernelFSHydrationPlan,
    ProjectionRunRequest,
};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

#[derive(Debug)]
struct MockCelldHttp {
    unary: Mutex<VecDeque<(String, Value)>>,
    stream: Mutex<VecDeque<(String, Value)>>,
}

impl MockCelldHttp {
    fn new() -> Self {
        Self {
            unary: Mutex::new(VecDeque::new()),
            stream: Mutex::new(VecDeque::new()),
        }
    }

    fn push_unary(&self, procedure: &str, response: Value) {
        self.unary
            .lock()
            .unwrap()
            .push_back((procedure.to_string(), response));
    }

    fn push_invoke_payload(&self, payload: Value) {
        let payload_b64 = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&payload).unwrap());
        let frame = json!({
            "payload": payload_b64,
            "contentType": "application/json",
            "done": true,
        });
        self.stream
            .lock()
            .unwrap()
            .push_back((GUEST_INVOKE.to_string(), frame));
    }
}

impl CelldHttpClient for MockCelldHttp {
    fn unary_json(
        &self,
        _base_url: &str,
        procedure: &str,
        _body: &[u8],
    ) -> lattice_cell_client::Result<(u16, Vec<u8>)> {
        let (expected, value) = self
            .unary
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| lattice_cell_client::CellClientError::Http("unexpected unary".into()))?;
        assert_eq!(expected, procedure);
        Ok((200, serde_json::to_vec(&value).unwrap()))
    }

    fn stream_json(
        &self,
        _base_url: &str,
        procedure: &str,
        _body: &[u8],
    ) -> lattice_cell_client::Result<(u16, Vec<u8>)> {
        let (expected, value) = self
            .stream
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| lattice_cell_client::CellClientError::Http("unexpected stream".into()))?;
        assert_eq!(expected, procedure);
        Ok((200, encode_connect_message(&value).unwrap()))
    }
}

struct ProposeResourceCapture {
    bodies: Arc<Mutex<Vec<Value>>>,
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
                "proposalId": "prop_cell",
                "status": "open"
            }))
    }
}

fn seed_celld_mock(http: &MockCelldHttp) {
    http.push_unary(
        CELL_APPLY,
        json!({
            "cell": {"id": "cell_demo", "observedState": "OBSERVED_STATE_READY"},
            "operation": {"operationId": "op_apply", "state": "OPERATION_STATE_SUCCEEDED"}
        }),
    );
    http.push_unary(
        CELL_START,
        json!({
            "operation": {"operationId": "op_start", "state": "OPERATION_STATE_SUCCEEDED"}
        }),
    );
    http.push_invoke_payload(json!({
        "state": "hydrated",
        "file_count": 1,
        "projection_id": "proj_demo"
    }));
    http.push_invoke_payload(json!({
        "state": "completed",
        "exit_code": 0,
        "projection_id": "proj_demo"
    }));
    let artifact = base64::engine::general_purpose::STANDARD.encode(b"cell output");
    http.push_invoke_payload(json!({
        "state": "collected",
        "file_count": 1,
        "files": [{
            "path": "output/out.txt",
            "sha256": "abc",
            "bytes": 11,
            "content_base64": artifact
        }]
    }));
}

struct CelldEnvGuard {
    previous: Option<String>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

static CELLD_ENV_LOCK: Mutex<()> = Mutex::new(());

impl CelldEnvGuard {
    fn set(url: &str) -> Self {
        let _lock = CELLD_ENV_LOCK.lock().expect("celld env lock");
        let previous = std::env::var(lattice_cell_client::CELLD_BASE_URL_ENV).ok();
        // SAFETY: tests restore env on drop; guarded by CELLD_ENV_LOCK.
        unsafe { std::env::set_var(lattice_cell_client::CELLD_BASE_URL_ENV, url) };
        Self {
            previous,
            _lock,
        }
    }

    fn unset_only() -> Self {
        let _lock = CELLD_ENV_LOCK.lock().expect("celld env lock");
        let previous = std::env::var(lattice_cell_client::CELLD_BASE_URL_ENV).ok();
        unsafe { std::env::remove_var(lattice_cell_client::CELLD_BASE_URL_ENV) };
        Self {
            previous,
            _lock,
        }
    }
}

impl Drop for CelldEnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests restore env on drop; mutex held until guard drops.
        unsafe {
            if let Some(value) = &self.previous {
                std::env::set_var(lattice_cell_client::CELLD_BASE_URL_ENV, value);
            } else {
                std::env::remove_var(lattice_cell_client::CELLD_BASE_URL_ENV);
            }
        }
    }
}

#[tokio::test]
async fn cell_output_map_proposes_via_latticed() {
    let http = MockCelldHttp::new();
    seed_celld_mock(&http);
    let celld = CelldClient::new("http://celld.test", http);

    let latticed = MockServer::start().await;
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let capture = ProposeResourceCapture {
        bodies: Arc::clone(&bodies),
    };
    Mock::given(method("POST"))
        .and(path("/v1/proposals/propose_resource"))
        .respond_with(capture)
        .expect(1)
        .mount(&latticed)
        .await;

    let lattice = LatticeToolClient::new(latticed.uri(), "test-token").expect("client");
    let workspace = WorkspaceBinding::new(Some("ws-cell".into()), None);
    let provenance = CellProposalProvenance {
        cell_id: "cell_demo".into(),
        projection_id: "proj_demo".into(),
        task_id: "proj_demo".into(),
        output_proposal_target: "Reports".into(),
    };

    let (_run, proposals) = run_cell_task_and_propose(
        &celld,
        &lattice,
        &workspace,
        &ProjectionRunRequest {
            cell_id: "cell_demo".into(),
            projection_id: "proj_demo".into(),
            plan: KernelFSHydrationPlan::from_role_paths("/tmp/in", None, "/tmp/out"),
            hydrate_files: vec![HydrateFile::text("input/hello.txt", "hi")],
            argv: vec!["/bin/sh".into(), "-c".into(), "echo ok".into()],
            ..ProjectionRunRequest::default()
        },
        "Reports",
        &provenance,
    )
    .await
    .expect("run and propose");

    assert_eq!(proposals.len(), 1);
    let captured = bodies.lock().expect("lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].get("path").and_then(|v| v.as_str()),
        Some("Reports/out.txt")
    );
    assert_eq!(
        captured[0].get("content").and_then(|v| v.as_str()),
        Some("cell output")
    );
    assert_eq!(
        captured[0].get("sourceResource").and_then(|v| v.as_str()),
        Some("cell://cell_demo/proj_demo")
    );
}

#[test]
fn output_map_to_drafts_strips_output_prefix() {
    use lattice_cell_client::OutputFile;
    use std::collections::BTreeMap;

    let mut map = BTreeMap::new();
    map.insert(
        "output/out.txt".into(),
        OutputFile {
            path: "output/out.txt".into(),
            sha256: String::new(),
            bytes: 3,
            content: b"abc".to_vec(),
        },
    );
    let drafts = output_map_to_drafts(&map, "Artifacts", "proj");
    assert_eq!(drafts[0].resource_path, "Artifacts/out.txt");
}

#[test]
fn run_cell_task_tool_absent_without_celld_url() {
    let _guard = CelldEnvGuard::unset_only();
    let defs = openai_tool_definitions();
    let names: Vec<_> = defs
        .iter()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .and_then(|value| value.as_str())
        })
        .collect();
    assert!(!names.contains(&"run_cell_task"));
}

#[test]
fn run_cell_task_tool_present_when_celld_url_set() {
    let _guard = CelldEnvGuard::set("http://127.0.0.1:8080");
    assert!(celld_configured());
    let defs = openai_tool_definitions();
    let names: Vec<_> = defs
        .iter()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .and_then(|value| value.as_str())
        })
        .collect();
    assert!(names.contains(&"run_cell_task"));
}

#[tokio::test]
async fn dispatch_run_cell_task_tool_end_to_end() {
    let workspace = tempfile::tempdir().expect("workspace");
    let input_path = workspace.path().join("input/hello.txt");
    std::fs::create_dir_all(input_path.parent().expect("parent")).expect("input dir");
    std::fs::write(&input_path, "hello").expect("write input");

    let latticed = MockServer::start().await;
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let capture = ProposeResourceCapture {
        bodies: Arc::clone(&bodies),
    };
    Mock::given(method("POST"))
        .and(path("/v1/proposals/propose_resource"))
        .respond_with(capture)
        .expect(1)
        .mount(&latticed)
        .await;

    let celld = MockServer::start().await;
    mount_celld_mocks(&celld).await;

    let _celld_guard = CelldEnvGuard::set(&celld.uri());

    let lattice = LatticeToolClient::new(latticed.uri(), "test-token").expect("client");
    let ctx = ToolRunContext {
        workspace_id: Some("ws-dispatch".into()),
        workspace_root: Some(workspace.path().to_string_lossy().into_owned()),
    };

    let args = json!({
        "cellId": "cell_demo",
        "projectionId": "proj_demo",
        "argv": ["/bin/sh", "-c", "echo ok"],
        "outputProposalTarget": "Reports",
        "hydrateResourcePaths": ["input/hello.txt"],
    })
    .to_string();

    let out = dispatch_tool(Some(&lattice), &ctx, "run_cell_task", &args).await;
    let parsed: Value = serde_json::from_str(&out).expect("tool json");
    assert!(
        parsed.get("error").is_none(),
        "unexpected tool error: {parsed}"
    );
    assert_eq!(parsed["cellId"], "cell_demo");
    assert_eq!(parsed["draftCount"], 1);
    assert_eq!(
        parsed["sourceResource"].as_str(),
        Some("cell://cell_demo/proj_demo")
    );

    let captured = bodies.lock().expect("lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].get("path").and_then(|v| v.as_str()),
        Some("Reports/out.txt")
    );
}

async fn mount_celld_mocks(server: &MockServer) {
    use lattice_cell_client::connect::{encode_connect_message, encode_unary_json, CELL_START};

    let apply_body = encode_unary_json(&json!({
        "cell": {"id": "cell_demo", "observedState": "OBSERVED_STATE_READY"},
        "operation": {"operationId": "op_apply", "state": "OPERATION_STATE_SUCCEEDED"}
    }))
    .unwrap();
    Mock::given(method("POST"))
        .and(path(CELL_APPLY))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(apply_body))
        .mount(server)
        .await;

    let start_body = encode_unary_json(&json!({
        "operation": {"operationId": "op_start", "state": "OPERATION_STATE_SUCCEEDED"}
    }))
    .unwrap();
    Mock::given(method("POST"))
        .and(path(CELL_START))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(start_body))
        .mount(server)
        .await;

    let invoke_frames: Vec<Vec<u8>> = [
        json!({
            "state": "hydrated",
            "file_count": 1,
            "projection_id": "proj_demo"
        }),
        json!({
            "state": "completed",
            "exit_code": 0,
            "projection_id": "proj_demo"
        }),
        json!({
            "state": "collected",
            "file_count": 1,
            "files": [{
                "path": "output/out.txt",
                "sha256": "abc",
                "bytes": 11,
                "content_base64": base64::engine::general_purpose::STANDARD.encode(b"cell output")
            }]
        }),
    ]
    .into_iter()
    .map(|payload| {
        let frame = json!({
            "payload": base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&payload).unwrap()),
            "contentType": "application/json",
            "done": true,
        });
        encode_connect_message(&frame).unwrap()
    })
    .collect();

    Mock::given(method("POST"))
        .and(path(GUEST_INVOKE))
        .respond_with(CelldInvokeSequence {
            frames: Mutex::new(VecDeque::from(invoke_frames)),
        })
        .expect(3)
        .mount(server)
        .await;
}

struct CelldInvokeSequence {
    frames: Mutex<VecDeque<Vec<u8>>>,
}

impl Respond for CelldInvokeSequence {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let frame = self
            .frames
            .lock()
            .unwrap()
            .pop_front()
            .expect("invoke response");
        ResponseTemplate::new(200).set_body_bytes(frame)
    }
}

#[tokio::test]
async fn dispatch_run_cell_task_errors_without_celld_url() {
    let _guard = CelldEnvGuard::unset_only();
    let lattice = MockServer::start().await;
    let client = LatticeToolClient::new(lattice.uri(), "test-token").expect("client");
    let ctx = ToolRunContext::default();
    let out = dispatch_tool(
        Some(&client),
        &ctx,
        "run_cell_task",
        &json!({
            "cellId": "c",
            "projectionId": "p",
            "argv": ["true"],
            "outputProposalTarget": "Reports"
        })
        .to_string(),
    )
    .await;
    let parsed: Value = serde_json::from_str(&out).expect("json");
    assert!(parsed["error"]
        .as_str()
        .is_some_and(|msg| msg.contains("CELLD_BASE_URL")));
}
