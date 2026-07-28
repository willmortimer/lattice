use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use kernelfs::{
    collect_output_commit_plan, configure_store, configure_wasi_preopens, engine_with_limits,
    materialize, materialize_with_options, normalize_guest_path, run_wasi_guest, Capabilities,
    ContentKind, ExecutionManifest, InputMount, LatticeProposalAdapter, MaterializeError,
    MaterializeOptions, Mounts, NetworkPolicy, SecretHandle, UnsupportedCapabilities,
    WasmtimeLimits, WasiPreopenSpec, WasiRunError, WasiRunOptions,
};
use wasmtime::{Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

fn copy_hello_wasm() -> &'static [u8] {
    include_bytes!("../fixtures/copy_hello.wasm")
}

fn spin_forever_wasm() -> &'static [u8] {
    include_bytes!("../fixtures/spin_forever.wasm")
}

fn stdio_exit_wasm() -> &'static [u8] {
    include_bytes!("../fixtures/stdio_exit.wasm")
}

fn empty_run(temp: &tempfile::TempDir, run_id: &str) -> PathBuf {
    let manifest = ExecutionManifest {
        run_id: run_id.into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: Vec::new(),
            output_proposal_target: None,
            work_promote_paths: Vec::new(),
        },
        capabilities: Default::default(),
    };
    materialize(temp.path(), &manifest).expect("materialize").root
}

#[test]
fn wasi_guest_reads_input_and_writes_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "hello from input").expect("write hello");

    let manifest = ExecutionManifest {
        run_id: "run_wasi_guest".into(),
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

    let run = materialize(temp.path(), &manifest).expect("materialize");
    let spec = WasiPreopenSpec::from_run_root(&run.root);

    let limits = WasmtimeLimits::default();
    let engine = engine_with_limits(&limits).expect("engine");
    let module = Module::from_binary(&engine, copy_hello_wasm()).expect("module");

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |ctx| ctx).expect("link wasi");
    let pre = linker.instantiate_pre(&module).expect("preinstantiate");

    let mut builder = WasiCtxBuilder::new();
    configure_wasi_preopens(&mut builder, &spec).expect("preopens");
    let wasi = builder.build_p1();

    let mut store = Store::new(&engine, wasi);
    configure_store(&mut store, &limits).expect("store limits");

    let instance = pre.instantiate(&mut store).expect("instantiate");
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .expect("_start export");
    start.call(&mut store, ()).expect("guest run");

    let out_path = run.root.join("output/out.txt");
    assert!(out_path.is_file(), "guest should write /output/out.txt");
    assert_eq!(
        fs::read(&out_path).expect("read output"),
        b"hello from input"
    );

    let plan = collect_output_commit_plan(&run.root, &manifest).expect("collect output");
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].relative_path, "out.txt");
    assert_eq!(plan.entries[0].content, b"hello from input");

    let adapter = LatticeProposalAdapter::from_manifest(&manifest);
    let drafts = adapter.drafts(&plan);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].resource_path, "Reports/out.txt");
}

#[test]
fn collect_output_includes_allowlisted_work_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = ExecutionManifest {
        run_id: "run_work_promote".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: Vec::new(),
            output_proposal_target: Some("Artifacts".into()),
            work_promote_paths: vec!["notes.txt".into(), "missing.txt".into()],
        },
        capabilities: Default::default(),
    };

    let run = materialize(temp.path(), &manifest).expect("materialize");
    fs::write(run.root.join("work/notes.txt"), "work artifact").expect("work file");
    fs::write(run.root.join("output/out.txt"), "output artifact").expect("output file");

    let plan = collect_output_commit_plan(&run.root, &manifest).expect("collect");
    assert_eq!(plan.entries.len(), 2);
    let paths: Vec<_> = plan.entries.iter().map(|e| e.relative_path.as_str()).collect();
    assert!(paths.contains(&"out.txt"));
    assert!(paths.contains(&"notes.txt"));
    for entry in &plan.entries {
        assert_eq!(entry.kind, ContentKind::Text);
    }
}

#[test]
fn collect_output_classifies_binary_and_promoted_work() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = ExecutionManifest {
        run_id: "run_binary_promote".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: Vec::new(),
            output_proposal_target: Some("Binaries".into()),
            work_promote_paths: vec!["blob.bin".into()],
        },
        capabilities: Default::default(),
    };

    let run = materialize(temp.path(), &manifest).expect("materialize");
    let binary = vec![0xff, 0xfe, 0x00, 0x01, 0x80];
    fs::write(run.root.join("output/raw.bin"), &binary).expect("output binary");
    fs::write(run.root.join("work/blob.bin"), &binary).expect("work binary");
    fs::write(run.root.join("output/note.txt"), "utf8 ok").expect("text");

    let plan = collect_output_commit_plan(&run.root, &manifest).expect("collect");
    assert_eq!(plan.entries.len(), 3);

    let raw = plan
        .entries
        .iter()
        .find(|e| e.relative_path == "raw.bin")
        .expect("raw.bin");
    assert_eq!(raw.kind, ContentKind::Bytes);
    assert_eq!(raw.content, binary);
    assert_eq!(
        raw.content_type_hint.as_deref(),
        Some("application/octet-stream")
    );

    let promoted = plan
        .entries
        .iter()
        .find(|e| e.relative_path == "blob.bin")
        .expect("blob.bin");
    assert_eq!(promoted.kind, ContentKind::Bytes);

    let note = plan
        .entries
        .iter()
        .find(|e| e.relative_path == "note.txt")
        .expect("note.txt");
    assert_eq!(note.kind, ContentKind::Text);

    let drafts = LatticeProposalAdapter::from_manifest(&manifest).drafts(&plan);
    assert!(drafts.iter().any(|d| d.kind == ContentKind::Bytes));
}

#[test]
fn materialize_rejects_network_allow_capability() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = ExecutionManifest {
        run_id: "run_net".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts::default(),
        capabilities: Capabilities {
            network: NetworkPolicy {
                allow: vec!["example.com".into()],
            },
            secrets: Vec::new(),
        },
    };

    let err = materialize(temp.path(), &manifest).unwrap_err();
    assert!(matches!(
        err,
        MaterializeError::UnsupportedCapabilities(UnsupportedCapabilities::NetworkAllow { .. })
    ));
}

#[test]
fn materialize_rejects_secrets_capability() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = ExecutionManifest {
        run_id: "run_secrets".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts::default(),
        capabilities: Capabilities {
            network: NetworkPolicy::default(),
            secrets: vec![SecretHandle {
                id: "vault://demo".into(),
            }],
        },
    };

    let err = materialize(temp.path(), &manifest).unwrap_err();
    assert!(matches!(
        err,
        MaterializeError::UnsupportedCapabilities(UnsupportedCapabilities::Secrets { .. })
    ));
}

#[test]
fn materialize_input_and_collect_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "hello from input").expect("write hello");

    let manifest = ExecutionManifest {
        run_id: "run_fixture".into(),
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

    let run = materialize(temp.path(), &manifest).expect("materialize");
    assert!(run.root.join("input/hello.txt").is_file());
    assert!(run.root.join("work").is_dir());
    assert!(run.root.join("output").is_dir());
    assert!(run.root.join("tmp").is_dir());
    assert_eq!(run.hydration.sources.len(), 1);
    assert_eq!(run.hydration.sources[0].guest_path, "hello.txt");

    let input_bytes = fs::read(run.root.join("input/hello.txt")).expect("read input");
    assert_eq!(input_bytes, b"hello from input");

    fs::write(run.root.join("output/out.txt"), "proposed output")
        .expect("simulate wasi write");

    let plan = collect_output_commit_plan(&run.root, &manifest).expect("collect output");
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].relative_path, "out.txt");

    let adapter = LatticeProposalAdapter::from_manifest(&manifest);
    let drafts = adapter.drafts(&plan);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].resource_path, "Reports/out.txt");
    assert_eq!(drafts[0].content, b"proposed output");
}

#[test]
fn rejects_parent_dir_in_input_guest_path() {
    let err = normalize_guest_path("../escape.txt").unwrap_err();
    assert!(matches!(err, MaterializeError::PathEscape { .. }));
}

#[test]
fn rejects_nested_parent_dir_escape() {
    let err = normalize_guest_path("nested/../../escape.txt").unwrap_err();
    assert!(matches!(err, MaterializeError::PathEscape { .. }));
}

#[test]
fn rejects_parent_dir_in_work_promote_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = ExecutionManifest {
        run_id: "run_bad_work".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: Vec::new(),
            output_proposal_target: None,
            work_promote_paths: vec!["../escape.txt".into()],
        },
        capabilities: Default::default(),
    };

    let run = materialize(temp.path(), &manifest).expect("materialize");
    let err = collect_output_commit_plan(&run.root, &manifest).unwrap_err();
    assert!(matches!(err, MaterializeError::PathEscape { .. }));
}

#[test]
fn manifest_round_trip_yaml() {
    let yaml = r#"
run_id: run_yaml
base_snapshot: snap_yaml
mounts:
  input:
    - host_path: /tmp/hello.txt
      guest_path: hello.txt
  output_proposal_target: Reports
"#;
    let manifest = ExecutionManifest::from_yaml(yaml).expect("parse yaml");
    assert_eq!(manifest.run_id, "run_yaml");
    assert_eq!(manifest.mounts.input.len(), 1);
    assert_eq!(
        manifest.mounts.output_proposal_target.as_deref(),
        Some("Reports")
    );
}

#[test]
fn wasi_preopen_spec_from_run_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("hello.txt");
    fs::write(&host_input, "x").expect("write");

    let manifest = ExecutionManifest {
        run_id: "run_wasi".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: host_input,
                guest_path: "hello.txt".into(),
            }],
            output_proposal_target: None,
            work_promote_paths: Vec::new(),
        },
        capabilities: Default::default(),
    };

    let run = materialize(temp.path(), &manifest).expect("materialize");
    let spec = WasiPreopenSpec::from_run_root(&run.root);
    assert!(spec.input.ends_with("input"));
    assert!(spec.work.ends_with("work"));
    assert!(spec.output.ends_with("output"));
    assert!(spec.tmp.ends_with("tmp"));

    let mut builder = WasiCtxBuilder::new();
    kernelfs::configure_wasi_preopens(&mut builder, &spec).expect("configure preopens");
}

#[test]
fn run_wasi_guest_helper_copies_input_to_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let host_input = temp.path().join("fixture-input");
    fs::create_dir_all(&host_input).expect("input dir");
    fs::write(host_input.join("hello.txt"), "via helper").expect("write hello");

    let manifest = ExecutionManifest {
        run_id: "run_helper".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: host_input.join("hello.txt"),
                guest_path: "hello.txt".into(),
            }],
            output_proposal_target: None,
            work_promote_paths: Vec::new(),
        },
        capabilities: Default::default(),
    };
    let run = materialize(temp.path(), &manifest).expect("materialize");

    let mut options = WasiRunOptions::default();
    // copy_hello finishes immediately; disable wall budget so a slow CI tick cannot flake.
    options.max_wall_time = None;
    options.limits = WasmtimeLimits::fuel_only(50_000_000);

    let result = run_wasi_guest(&run.root, copy_hello_wasm(), &options).expect("run");
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        fs::read(run.root.join("output/out.txt")).expect("out"),
        b"via helper"
    );
}

#[test]
fn run_wasi_guest_captures_stdio_and_exit_code() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = empty_run(&temp, "run_stdio");

    let mut options = WasiRunOptions::default();
    options.max_wall_time = None;
    options.limits = WasmtimeLimits::fuel_only(50_000_000);

    let result = run_wasi_guest(&root, stdio_exit_wasm(), &options).expect("run");
    assert_eq!(result.exit_code, 7);
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("hello-stdout"),
        "stdout={:?}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("boom-from-stderr"),
        "stderr={:?}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn run_wasi_guest_cancels_runaway() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = empty_run(&temp, "run_cancel");

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_flag = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        cancel_flag.store(true, Ordering::SeqCst);
    });

    let options = WasiRunOptions {
        limits: WasmtimeLimits::epoch_only(1),
        epoch_tick_interval: Duration::from_millis(5),
        max_wall_time: None,
        cancel: Some(cancel),
        ..WasiRunOptions::default()
    };

    let err = run_wasi_guest(&root, spin_forever_wasm(), &options).unwrap_err();
    assert!(
        matches!(err, WasiRunError::Cancelled { .. }),
        "expected Cancelled, got {err:?}"
    );
}

#[test]
fn run_wasi_guest_hits_epoch_wall_deadline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = empty_run(&temp, "run_epoch");

    let options = WasiRunOptions {
        limits: WasmtimeLimits::epoch_only(1),
        epoch_tick_interval: Duration::from_millis(5),
        max_wall_time: Some(Duration::from_millis(40)),
        cancel: None,
        ..WasiRunOptions::default()
    };

    let err = run_wasi_guest(&root, spin_forever_wasm(), &options).unwrap_err();
    assert!(
        matches!(err, WasiRunError::EpochDeadline { .. }),
        "expected EpochDeadline, got {err:?}"
    );
}

#[test]
fn run_wasi_guest_hits_fuel_limit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = empty_run(&temp, "run_fuel");

    let options = WasiRunOptions {
        limits: WasmtimeLimits::fuel_only(1_000),
        max_wall_time: None,
        cancel: None,
        ..WasiRunOptions::default()
    };

    let err = run_wasi_guest(&root, spin_forever_wasm(), &options).unwrap_err();
    assert!(
        matches!(err, WasiRunError::FuelExhausted { .. }),
        "expected FuelExhausted, got {err:?}"
    );
}

#[test]
fn materialize_rejects_host_path_outside_allowlist() {
    let temp = tempfile::tempdir().expect("tempdir");
    let allowed = temp.path().join("allowed");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&allowed).expect("allowed");
    fs::create_dir_all(&outside).expect("outside");
    fs::write(allowed.join("ok.txt"), "ok").expect("ok");
    fs::write(outside.join("secret.txt"), "nope").expect("secret");

    let manifest = ExecutionManifest {
        run_id: "run_allow".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: outside.join("secret.txt"),
                guest_path: "secret.txt".into(),
            }],
            output_proposal_target: None,
            work_promote_paths: Vec::new(),
        },
        capabilities: Default::default(),
    };

    let opts = MaterializeOptions {
        host_path_roots: &[allowed],
    };
    let err = materialize_with_options(temp.path(), &manifest, &opts).unwrap_err();
    assert!(matches!(err, MaterializeError::HostPathNotAllowed { .. }));
}

#[test]
fn materialize_allows_host_path_under_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let allowed = temp.path().join("workspace");
    fs::create_dir_all(&allowed).expect("workspace");
    fs::write(allowed.join("ok.txt"), "ok").expect("ok");

    let manifest = ExecutionManifest {
        run_id: "run_allow_ok".into(),
        base_snapshot: "snap".into(),
        mounts: Mounts {
            input: vec![InputMount {
                host_path: allowed.join("ok.txt"),
                guest_path: "ok.txt".into(),
            }],
            output_proposal_target: None,
            work_promote_paths: Vec::new(),
        },
        capabilities: Default::default(),
    };

    let opts = MaterializeOptions {
        host_path_roots: &[allowed.clone()],
    };
    let run = materialize_with_options(temp.path(), &manifest, &opts).expect("materialize");
    assert_eq!(
        fs::read(run.root.join("input/ok.txt")).expect("read"),
        b"ok"
    );
}
