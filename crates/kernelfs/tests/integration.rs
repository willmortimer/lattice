use std::fs;

use kernelfs::{
    collect_output_commit_plan, configure_store, configure_wasi_preopens, engine_with_limits,
    materialize, normalize_guest_path, ExecutionManifest, InputMount, LatticeProposalAdapter,
    MaterializeError, Mounts, WasmtimeLimits, WasiPreopenSpec,
};
use wasmtime::{Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

fn copy_hello_wasm() -> &'static [u8] {
  include_bytes!("../fixtures/copy_hello.wasm")
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
