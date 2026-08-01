//! KernelFS WASI guest → Lattice proposal drafts → mocked propose_resource.

use std::fs;
use std::sync::Arc;

use kernelfs::{
    Capabilities, ContentKind, ExecutionManifest, InputMount, LatticeProposalDraft, Mounts,
    NetworkPolicy, SecretHandle, SecretHandleEntry, WasmtimeLimits,
};
use base64::Engine;
use lattice_agentd::lattice_client::LatticeToolClient;
use lattice_agentd::tools::{dispatch_tool, openai_tool_definitions, ToolRunContext};
use lattice_agentd::wasi_host::{
    hydration_inputs_from_record, propose_output_drafts, resolve_hydration_resource_ids,
    run_wasi_guest_with_options, WasiGuestHostOptions, WorkspaceBinding,
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

struct RunEventCapture {
    bodies: Arc<std::sync::Mutex<Vec<Value>>>,
}

impl Respond for RunEventCapture {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if let Ok(body) = serde_json::from_slice::<Value>(&request.body) {
            if let Ok(mut guard) = self.bodies.lock() {
                guard.push(body);
            }
        }
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_json(json!({
                "workspaceId": "ws-dispatch",
                "event": {
                    "eventSequence": 1,
                    "eventType": "run.created",
                },
                "run": { "status": "running" },
            }))
    }
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

/// SHA-256 of fixture payload `hello from input` (KernelFS copy_hello input).
const HELLO_FROM_INPUT_SHA256: &str =
    "0f328ae687eb8fd2acfa3a910bb6722eff43f8a7dbd08e53e572ae37a0c5d7a5";

#[tokio::test]
async fn wasi_propose_includes_hydration_digest_with_known_hash() {
    ensure_seatbelt_runner();
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "hello from input").expect("write hello");

    let manifest = ExecutionManifest {
        run_id: "run_prov_hash".into(),
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

    assert_eq!(result.hydration.sources.len(), 1);
    assert_eq!(result.hydration.sources[0].guest_path, "hello.txt");
    assert_eq!(
        result.hydration.sources[0].sha256, HELLO_FROM_INPUT_SHA256,
        "hydration sha256 must match fixture"
    );

    let mut resource_ids = std::collections::BTreeMap::new();
    resource_ids.insert("hello.txt".into(), "res-fixture-1".into());
    let provenance = lattice_agentd::WasiProposalProvenance {
        run_id: "run_prov_hash".into(),
        wasm_path: "Tools/guests/copy_hello.wasm".into(),
        output_proposal_target: "Reports".into(),
        hydration_inputs: lattice_agentd::hydration_inputs_from_record(
            &result.hydration,
            &resource_ids,
        ),
    };

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
    lattice_agentd::propose_output_drafts_with_provenance(
        &client,
        &WorkspaceBinding::new(Some("ws-prov".into()), None),
        &result.drafts,
        Some(&provenance),
    )
    .await
    .expect("propose with provenance");

    let captured = bodies.lock().expect("lock bodies");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].get("sourceResource").and_then(|v| v.as_str()),
        Some("wasi://run_prov_hash/Tools/guests/copy_hello.wasm")
    );
    let inputs = captured[0]
        .get("hydrationInputs")
        .and_then(|v| v.as_array())
        .expect("hydrationInputs array");
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0]["path"], "hello.txt");
    assert_eq!(inputs[0]["contentHash"], HELLO_FROM_INPUT_SHA256);
    assert_eq!(inputs[0]["resourceId"], "res-fixture-1");
}

#[tokio::test]
async fn wasi_propose_resolves_resource_id_from_registry_without_explicit_map() {
    ensure_seatbelt_runner();
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace_path = "input/hello.txt";
    std::fs::create_dir_all(temp.path().join("input")).expect("input dir");
    std::fs::write(temp.path().join(workspace_path), "hello from input").expect("write hello");

    let mut registry = latticefs_core::NamespaceRegistry::open(temp.path()).expect("registry");
    let resource_id = registry
        .ensure_local_file(workspace_path)
        .expect("register input");
    registry.save().expect("save registry");

    let manifest = ExecutionManifest {
        run_id: "run_prov_registry".into(),
        base_snapshot: "snap_1".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: temp.path().join(workspace_path),
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

    let mut guest_to_workspace = std::collections::BTreeMap::new();
    guest_to_workspace.insert("hello.txt".into(), workspace_path.into());
    let resource_ids = resolve_hydration_resource_ids(
        Some(temp.path().to_str().expect("workspace path")),
        &std::collections::BTreeMap::new(),
        &guest_to_workspace,
    );
    assert_eq!(
        resource_ids.get("hello.txt").map(String::as_str),
        Some(resource_id.to_string().as_str())
    );

    let provenance = lattice_agentd::WasiProposalProvenance {
        run_id: "run_prov_registry".into(),
        wasm_path: "Tools/guests/copy_hello.wasm".into(),
        output_proposal_target: "Reports".into(),
        hydration_inputs: hydration_inputs_from_record(&result.hydration, &resource_ids),
    };

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
    lattice_agentd::propose_output_drafts_with_provenance(
        &client,
        &WorkspaceBinding::new(Some("ws-prov".into()), Some(temp.path().to_string_lossy().into())),
        &result.drafts,
        Some(&provenance),
    )
    .await
    .expect("propose with provenance");

    let captured = bodies.lock().expect("lock bodies");
    let inputs = captured[0]
        .get("hydrationInputs")
        .and_then(|v| v.as_array())
        .expect("hydrationInputs array");
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0]["path"], "hello.txt");
    assert_eq!(inputs[0]["contentHash"], HELLO_FROM_INPUT_SHA256);
    assert_eq!(
        inputs[0]["resourceId"].as_str(),
        Some(resource_id.to_string().as_str())
    );
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
        thread_id: None,
    };

    let args = json!({
        "preset": "copy_hello",
        "resourcePaths": ["input/hello.txt"],
        "outputProposalTarget": "Reports",
        "runId": "run_dispatch_test",
    })
    .to_string();

    let out = dispatch_tool(Some(&client), &ctx, None, "run_wasi_guest", &args).await;
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

#[tokio::test]
async fn dispatch_run_wasi_guest_emits_lifecycle_events() {
    ensure_seatbelt_runner();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let wasm_dest = workspace.path().join("Tools/guests/copy_hello.wasm");
    fs::create_dir_all(wasm_dest.parent().expect("wasm parent")).expect("wasm dir");
    fs::write(&wasm_dest, copy_hello_wasm()).expect("write wasm");

    let input_path = workspace.path().join("input/hello.txt");
    fs::create_dir_all(input_path.parent().expect("input parent")).expect("input dir");
    fs::write(&input_path, "hello from input").expect("write input");

    let latticed = MockServer::start().await;
    let event_bodies = Arc::new(std::sync::Mutex::new(Vec::new()));
    let event_capture = RunEventCapture {
        bodies: Arc::clone(&event_bodies),
    };

    Mock::given(method("POST"))
        .and(path("/v1/agent_runs/run_lifecycle_test/events"))
        .respond_with(event_capture)
        .mount(&latticed)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/proposals/propose_resource"))
        .respond_with(ProposeResourceCapture {
            bodies: Arc::new(std::sync::Mutex::new(Vec::new())),
        })
        .mount(&latticed)
        .await;

    let client = LatticeToolClient::new(latticed.uri(), "test-token").expect("client");
    let ctx = ToolRunContext {
        workspace_id: Some("ws-dispatch".into()),
        workspace_root: Some(workspace.path().to_string_lossy().into_owned()),
        thread_id: Some("thread-lifecycle".into()),
    };

    let args = json!({
        "preset": "copy_hello",
        "resourcePaths": ["input/hello.txt"],
        "outputProposalTarget": "Reports",
        "runId": "run_lifecycle_test",
    })
    .to_string();

    let out = dispatch_tool(Some(&client), &ctx, None, "run_wasi_guest", &args).await;
    let parsed: Value = serde_json::from_str(&out).expect("tool result json");
    assert!(
        parsed.get("error").is_none(),
        "unexpected tool error: {parsed}"
    );

    let captured = event_bodies.lock().expect("lock event bodies");
    let event_types: Vec<&str> = captured
        .iter()
        .filter_map(|body| body.get("eventType").and_then(|v| v.as_str()))
        .collect();
    assert!(
        event_types.contains(&"run.created"),
        "missing run.created: {event_types:?}"
    );
    assert!(
        event_types.contains(&"run.hydrating"),
        "missing run.hydrating: {event_types:?}"
    );
    assert!(
        event_types.contains(&"run.executing"),
        "missing run.executing: {event_types:?}"
    );
    assert!(
        event_types.contains(&"run.output_available"),
        "missing run.output_available: {event_types:?}"
    );
    assert!(
        event_types.contains(&"run.proposal_ready"),
        "missing run.proposal_ready: {event_types:?}"
    );
    assert!(
        event_types.contains(&"run.released"),
        "missing run.released: {event_types:?}"
    );
    assert_eq!(
        captured[0].get("threadId").and_then(|v| v.as_str()),
        Some("thread-lifecycle")
    );
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
    assert!(props.get("secretHandlesJson").is_some());
    let required = wasi
        .pointer("/function/parameters/required")
        .and_then(|v| v.as_array())
        .expect("required");
    assert!(required
        .iter()
        .any(|v| v.as_str() == Some("outputProposalTarget")));
}

#[tokio::test]
async fn secret_handles_deny_by_default() {
    ensure_seatbelt_runner();
    let temp = tempfile::tempdir().expect("tempdir");
    let secret_file = temp.path().join("api-key.txt");
    fs::write(&secret_file, "super-secret").expect("write secret");

    let manifest = ExecutionManifest {
        run_id: "run_secret_deny".into(),
        base_snapshot: "snap".into(),
        mounts: Default::default(),
        capabilities: Capabilities {
            secrets: vec![SecretHandle {
                id: "api-key".into(),
            }],
            ..Default::default()
        },
    };

    let run_parent = temp.path().to_path_buf();
    let host_roots = vec![temp.path().to_path_buf()];
    let err = tokio::task::spawn_blocking(move || {
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
    .expect_err("secret without allowlist should fail");

    let materialize_err = match err {
        lattice_agentd::WasiHostError::Materialize(inner) => inner,
        other => panic!("expected materialize failure, got {other:?}"),
    };
    let error_json = lattice_agentd::wasi_materialize_error_json(&materialize_err);
    assert_eq!(error_json["kind"], "secret_not_allowed");
    assert_eq!(error_json["secretId"], "api-key");
    let message = error_json["message"].as_str().expect("message");
    assert!(message.contains("secret"), "{message}");
    assert!(message.contains("api-key"), "{message}");
    assert!(message.contains("secretHandlesJson"), "{message}");
}

#[tokio::test]
async fn network_allow_rejects_with_clear_capability_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = ExecutionManifest {
        run_id: "run_network_deny".into(),
        base_snapshot: "snap".into(),
        mounts: Default::default(),
        capabilities: Capabilities {
            network: NetworkPolicy {
                allow: vec!["example.com".into()],
            },
            ..Default::default()
        },
    };

    let run_parent = temp.path().to_path_buf();
    let host_roots = vec![temp.path().to_path_buf()];
    let err = tokio::task::spawn_blocking(move || {
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
    .expect_err("network.allow should fail closed at materialize");

    let materialize_err = match err {
        lattice_agentd::WasiHostError::Materialize(inner) => inner,
        other => panic!("expected materialize failure, got {other:?}"),
    };
    let error_json = lattice_agentd::wasi_materialize_error_json(&materialize_err);
    assert_eq!(error_json["kind"], "unsupported_capability");
    assert_eq!(error_json["capability"], "network.allow");
    let message = error_json["message"].as_str().expect("message");
    assert!(message.contains("network.allow"), "{message}");
    assert!(message.contains("example.com"), "{message}");
    assert!(message.contains("host tools"), "{message}");
}

#[tokio::test]
async fn secret_handles_materialize_when_allowlisted() {
    ensure_seatbelt_runner();
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "hello from input").expect("write hello");

    let secret_file = temp.path().join("api-key.txt");
    fs::write(&secret_file, "super-secret").expect("write secret");

    let manifest = ExecutionManifest {
        run_id: "run_secret_ok".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: host_input.join("hello.txt"),
                guest_path: "hello.txt".into(),
            }],
            output_proposal_target: Some("Reports".into()),
            work_promote_paths: Vec::new(),
        },
        capabilities: Capabilities {
            secrets: vec![SecretHandle {
                id: "api-key".into(),
            }],
            ..Default::default()
        },
    };

    let run_parent = temp.path().to_path_buf();
    let host_roots = vec![temp.path().to_path_buf()];
    let allowlist = vec![SecretHandleEntry {
        id: "api-key".into(),
        host_path: secret_file,
    }];
    let result = tokio::task::spawn_blocking(move || {
        run_wasi_guest_with_options(
            &run_parent,
            &manifest,
            copy_hello_wasm(),
            &WasiGuestHostOptions {
                limits: WasmtimeLimits::default(),
                host_path_roots: host_roots,
                secret_handle_allowlist: allowlist,
                ..Default::default()
            },
        )
    })
    .await
    .expect("join")
    .expect("allowlisted secret should run");

    assert_eq!(result.drafts.len(), 1);
    let secret_path = temp
        .path()
        .join("run_secret_ok/run/secrets/api-key");
    assert!(
        secret_path.is_file(),
        "expected secret at {}",
        secret_path.display()
    );
    assert_eq!(
        fs::read(&secret_path).expect("read secret"),
        b"super-secret"
    );
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
