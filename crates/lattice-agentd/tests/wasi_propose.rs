//! KernelFS WASI guest → Lattice proposal drafts → mocked propose_resource.

use std::fs;
use std::sync::Arc;

use kernelfs::{
    ContentKind, ExecutionManifest, InputMount, LatticeProposalDraft, Mounts, WasmtimeLimits,
};
use base64::Engine;
use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::tools::{dispatch_tool, openai_tool_definitions, ToolRunContext};
use lattice_agentd::wasi_host::{
    propose_output_drafts, run_wasi_guest_with_options, WasiGuestHostOptions, WorkspaceBinding,
};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn copy_hello_wasm() -> &'static [u8] {
    include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../kernelfs/crates/kernelfs/fixtures/copy_hello.wasm"
))
}

fn ensure_seatbelt_runner() {
    // Prefer the package helper built alongside integration tests.
    std::env::set_var(
        lattice_agentd::seatbelt::SEATBELT_BIN_ENV,
        env!("CARGO_BIN_EXE_lattice-wasi-seatbelt"),
    );
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
    ensure_seatbelt_runner();
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
    let host_roots = vec![temp.path().to_path_buf()];
    let result = tokio::task::spawn_blocking(move || {
        run_wasi_guest_with_options(
            &run_parent,
            &manifest,
            copy_hello_wasm(),
            &WasiGuestHostOptions {
                limits: WasmtimeLimits::default(),
                host_path_roots: host_roots,
                ..Default::default()
            },
        )
    })
    .await
    .expect("join wasi")
    .expect("run wasi guest");

    let drafts = result.drafts;
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

#[tokio::test]
async fn binary_output_draft_proposes_via_content_base64() {
    let binary = vec![0xff_u8, 0xfe, 0x00, 0x01, 0x80];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&binary);
    let drafts = vec![LatticeProposalDraft {
        summary: "Create resource Reports/raw.bin from KernelFS run run_bin".into(),
        resource_path: "Reports/raw.bin".into(),
        content: binary.clone(),
        kind: ContentKind::Bytes,
    }];

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
        &WorkspaceBinding::new(Some("ws-bin".into()), None),
        &drafts,
    )
    .await
    .expect("propose binary draft");

    assert_eq!(responses.len(), 1);
    let captured = bodies.lock().expect("lock bodies");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].get("workspaceId").and_then(|v| v.as_str()),
        Some("ws-bin")
    );
    assert_eq!(
        captured[0].get("path").and_then(|v| v.as_str()),
        Some("Reports/raw.bin")
    );
    assert!(captured[0].get("content").is_none());
    assert_eq!(
        captured[0].get("contentBase64").and_then(|v| v.as_str()),
        Some(encoded.as_str())
    );
}

#[tokio::test]
async fn dispatch_run_wasi_guest_tool_proposes_outputs() {
    ensure_seatbelt_runner();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let wasm_dest = workspace.path().join("Tools/guests/copy_hello.wasm");
    fs::create_dir_all(wasm_dest.parent().expect("wasm parent")).expect("wasm dir");
    fs::write(&wasm_dest, copy_hello_wasm()).expect("write wasm");

    let input_path = workspace.path().join("input/hello.txt");
    fs::create_dir_all(input_path.parent().expect("input parent")).expect("input dir");
    fs::write(&input_path, "hello from input").expect("write input");

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
    let ctx = ToolRunContext {
        workspace_id: Some("ws-dispatch".into()),
        workspace_root: Some(workspace.path().to_string_lossy().into_owned()),
    };

    let args = json!({
        "preset": "copy_hello",
        "resourcePaths": ["input/hello.txt"],
        "outputProposalTarget": "Reports",
        "runId": "run_dispatch_test",
    })
    .to_string();

    let out = dispatch_tool(Some(&client), &ctx, "run_wasi_guest", &args).await;
    let parsed: Value = serde_json::from_str(&out).expect("tool result json");
    assert!(
        parsed.get("error").is_none(),
        "unexpected tool error: {parsed}"
    );
    assert_eq!(parsed["runId"], "run_dispatch_test");
    assert_eq!(parsed["draftCount"], 1);
    assert_eq!(
        parsed["sourceResource"].as_str(),
        Some("wasi://run_dispatch_test/Tools/guests/copy_hello.wasm")
    );
    assert_eq!(
        parsed["proposals"][0].get("proposalId").and_then(|v| v.as_str()),
        Some("prop_test")
    );
    assert_eq!(
        parsed["proposals"][0].get("path").and_then(|v| v.as_str()),
        Some("Reports/out.txt")
    );

    let captured = bodies.lock().expect("lock bodies");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].get("workspaceId").and_then(|v| v.as_str()),
        Some("ws-dispatch")
    );
    assert_eq!(
        captured[0].get("sourceResource").and_then(|v| v.as_str()),
        Some("wasi://run_dispatch_test/Tools/guests/copy_hello.wasm")
    );
    let summary = captured[0]
        .get("summary")
        .and_then(|v| v.as_str())
        .expect("summary");
    assert!(summary.contains("runId=run_dispatch_test"));
    assert!(summary.contains("target=Reports"));
    assert!(summary.contains("hello.txt@"));
}

#[test]
fn run_wasi_guest_tool_schema_documents_presets() {
    let tools = openai_tool_definitions();
    let wasi = tools
        .iter()
        .find(|tool| {
            tool.pointer("/function/name")
                .and_then(|v| v.as_str())
                == Some("run_wasi_guest")
        })
        .expect("run_wasi_guest tool");
    let props = wasi
        .pointer("/function/parameters/properties")
        .expect("properties");
    assert!(props.get("preset").is_some());
    assert!(props.get("resourcePaths").is_some());
    assert!(props.get("workPromotePaths").is_some());
    let required = wasi
        .pointer("/function/parameters/required")
        .and_then(|v| v.as_array())
        .expect("required");
    assert!(required
        .iter()
        .any(|v| v.as_str() == Some("outputProposalTarget")));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_seatbelt_writes_profile_and_runs_guest() {
    ensure_seatbelt_runner();
    std::env::set_var(lattice_agentd::seatbelt::SEATBELT_ENV, "1");

    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "hello from input").expect("write hello");

    let manifest = ExecutionManifest {
        run_id: "run_seatbelt".into(),
        base_snapshot: "snap".into(),
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
    let host_roots = vec![temp.path().to_path_buf()];
    let result = tokio::task::spawn_blocking(move || {
        run_wasi_guest_with_options(
            &run_parent,
            &manifest,
            copy_hello_wasm(),
            &WasiGuestHostOptions {
                limits: WasmtimeLimits::default(),
                host_path_roots: host_roots,
                ..Default::default()
            },
        )
    })
    .await
    .expect("join")
    .expect("seatbelt wasi");

    assert_eq!(result.drafts.len(), 1);
    let profile = temp.path().join("run_seatbelt/.host/seatbelt.sb");
    assert!(
        profile.is_file(),
        "expected Seatbelt profile at {}",
        profile.display()
    );
    let profile_text = fs::read_to_string(&profile).expect("read profile");
    assert!(profile_text.contains("(deny network*)"));
    assert!(profile_text.contains("lattice-wasi-seatbelt"));
}
