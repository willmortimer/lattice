//! Canonical governed-loop integration smoke (T3).
//!
//! Provisions the First Look `demo` template into a temp workspace, then walks
//! form insert → workflow run → proposal preview/apply → derived rebuild → undo
//! with persisted operational artifacts asserted at each boundary.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use rusqlite::Connection;

use lattice_commands::{
    apply_proposal, create_proposal, discover_workflows, list_proposal_summaries,
    list_relationship_edges, load_and_run_workflow, load_derived_status, load_lineage,
    load_proposal, preview_proposal, rebuild_derived, workflow_runs_dir, lineage_path, Command, CommandEngine,
    DerivedState, DerivedStaleReason, ExecutionStatus, ProposalSourceType, ProposalStatus,
    RelationshipKind, TaskRunner, Transaction, TransactionProposal, WorkflowRunRecord,
};
use lattice_core::init_with_template;
use lattice_data::CellValue;
use lattice_env::EnvProvider;
use tempfile::TempDir;

const DEMO_TEMPLATE: &str = "demo";
const CRM_PACKAGE: &str = "CRM.data";
const CONTACT_FORM: &str = "ContactIntake";
const CONTACT_TABLE: &str = "contacts";
const INTAKE_WORKFLOW: &str = "Automations/Contact intake.workflow.yaml";
const PROPOSAL_PAGE: &str = "Proposals/Contact intake follow-up.md";
const DERIVED_RESOURCE: &str = "Derived/ContactBrief.derived.yaml";
const DERIVED_OUTPUT: &str = "Derived/dist/index.html";

static PATH_LOCK: Mutex<()> = Mutex::new(());

struct PathPrefixGuard {
    previous: Option<String>,
}

impl PathPrefixGuard {
    fn prepend(dir: &Path) -> Self {
        let previous = std::env::var("PATH").ok();
        let joined = match &previous {
            Some(prev) => format!("{}:{}", dir.display(), prev),
            None => dir.display().to_string(),
        };
        std::env::set_var("PATH", joined);
        Self { previous }
    }
}

impl Drop for PathPrefixGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(prev) => std::env::set_var("PATH", prev),
            None => std::env::remove_var("PATH"),
        }
    }
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write shim");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod shim");
}

/// Hermetic `uv` + `python3` shims so demo tasks run without network installs.
fn install_task_shims(bin: &Path) -> PathBuf {
    fs::create_dir_all(bin).expect("create bin");
    let python = bin.join("python3");
    write_executable(
        &python,
        "#!/bin/sh\nexec /usr/bin/env python3 \"$@\"\n",
    );
    let uv = bin.join("uv");
    write_executable(
        &uv,
        &format!(
            r#"#!/bin/sh
PYTHON_BIN="{python}"
if [ "$1" = "python" ] && [ "$2" = "find" ]; then
  printf '%s\n' "$PYTHON_BIN"
  exit 0
fi
if [ "$1" = "run" ]; then
  shift
  project_dir=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --directory) project_dir="$2"; shift 2 ;;
      --) shift; break ;;
      *) shift ;;
    esac
  done
  if [ -n "$project_dir" ]; then
    cd "$project_dir" || exit 1
  fi
  exec "$@"
fi
echo "fake uv: unsupported: $*" >&2
exit 1
"#,
            python = python.display()
        ),
    );
    python
}

fn log_boundary(name: &str, detail: &str) {
    eprintln!("[governed-loop] {name}: {detail}");
}

fn await_until<F>(label: &str, mut check: F, attempts: usize) -> bool
where
    F: FnMut() -> bool,
{
    for attempt in 0..attempts {
        if check() {
            if attempt > 0 {
                log_boundary(label, &format!("ready after {attempt} polls"));
            }
            return true;
        }
        thread::yield_now();
    }
    false
}

fn contact_count(root: &Path) -> usize {
    lattice_data::DataApp::open(&root.join(CRM_PACKAGE))
        .expect("open CRM.data")
        .list_rows(CONTACT_TABLE, 10_000, 0)
        .expect("list contacts")
        .len()
}

fn history_transaction_count(root: &Path) -> usize {
    let db = root.join(".lattice/history.sqlite");
    let conn = Connection::open(db).expect("open history.sqlite");
    conn.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))
        .expect("count transactions")
}

fn latest_history_id(root: &Path) -> String {
    CommandEngine::open(root)
        .expect("open engine for history")
        .history(1)
        .expect("history")[0]
        .id
        .clone()
}

fn run_form_submitted_workflows(root: &Path) -> Vec<WorkflowRunRecord> {
    let form_file = format!("{CRM_PACKAGE}/forms/{CONTACT_FORM}.form.yaml");
    let mut records = Vec::new();
    for (path, manifest) in discover_workflows(root).expect("discover workflows") {
        if !manifest.enabled {
            continue;
        }
        if !manifest.matches_form_submitted(CRM_PACKAGE, CONTACT_FORM, Some(&form_file)) {
            continue;
        }
        let record = load_and_run_workflow(root, &path, Some("form.submitted"), None, None)
            .unwrap_or_else(|err| {
                panic!(
                    "workflow run failed for {}: {err}",
                    path.display()
                )
            });
        records.push(record);
    }
    records
}

fn provision_demo_workspace() -> TempDir {
    let dir = tempfile::tempdir().expect("temp workspace");
    init_with_template(dir.path(), "Governed loop smoke", DEMO_TEMPLATE)
        .unwrap_or_else(|err| panic!("provision demo template: {err}"));
    dir
}

#[test]
fn governed_loop_smoke_demo_template() {
    let _path_guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let shim_dir = tempfile::tempdir().expect("shim dir");
    let _path_prefix = PathPrefixGuard::prepend(shim_dir.path());
    install_task_shims(shim_dir.path());

    let workspace = provision_demo_workspace();
    let root = workspace.path();

    // 1. Form / record insert committed via the command engine.
    let contacts_before = contact_count(root);
    let mut engine = CommandEngine::open(root).expect("open engine");
    let insert_receipt = engine
        .apply(Transaction::new(
            format!("Insert row into {CRM_PACKAGE}.{CONTACT_TABLE}"),
            vec![Command::RecordInsert {
                path: PathBuf::from(CRM_PACKAGE),
                table: CONTACT_TABLE.into(),
                values: BTreeMap::from([
                    ("name".into(), CellValue::Text("Smoke Test".into())),
                    ("email".into(), CellValue::Text("smoke@lattice.local".into())),
                    ("status".into(), CellValue::Text("new".into())),
                ]),
                id: None,
            }],
        ))
        .expect("record insert");
    let record_id = insert_receipt.outcomes[0]
        .resulting_record_id
        .clone()
        .expect("insert record id");
    assert_eq!(contact_count(root), contacts_before + 1);
    assert!(
        root.join(".lattice/history.sqlite").is_file(),
        "history.sqlite missing after insert"
    );
    let insert_tx = latest_history_id(root);
    log_boundary(
        "record-insert",
        &format!("record_id={record_id} transaction_id={insert_tx}"),
    );

    // 2. Workflow run persisted for form.submitted.
    let runs = run_form_submitted_workflows(root);
    assert_eq!(
        runs.len(),
        1,
        "expected exactly one matching intake workflow"
    );
    let run = &runs[0];
    assert_eq!(run.workflow_path, INTAKE_WORKFLOW);
    assert_eq!(run.trigger, "form.submitted");
    assert_eq!(run.execution.status, ExecutionStatus::Succeeded);
    let execution_id = run.execution.id.clone();
    let run_path = workflow_runs_dir(root).join(format!("{execution_id}.json"));
    assert!(run_path.is_file(), "missing workflow run file at {}", run_path.display());
    log_boundary(
        "workflow-run",
        &format!("execution_id={execution_id} run_file={}", run_path.display()),
    );

    // 3–4. Task/proposal step succeeded; exactly one pending proposal (idempotent).
    let proposal_id = run
        .execution
        .proposal_id
        .clone()
        .expect("workflow proposal id");
    assert_eq!(run.execution.proposal_ids, vec![proposal_id.clone()]);
    assert!(
        run.steps.iter().any(|step| step.id == "run-hello" && step.status == ExecutionStatus::Succeeded),
        "task step run-hello did not succeed: {:?}",
        run.steps
    );
    assert!(
        run.steps.iter().any(|step| step.id == "create-proposal" && step.status == ExecutionStatus::Succeeded),
        "proposal step create-proposal did not succeed: {:?}",
        run.steps
    );
    let summaries = list_proposal_summaries(root).expect("list proposals");
    assert_eq!(summaries.len(), 1, "expected one pending proposal");
    let proposal_path = root
        .join(".lattice/proposals")
        .join(format!("{proposal_id}.json"));
    assert!(proposal_path.is_file(), "missing proposal file");
    let proposal = load_proposal(root, &proposal_id).expect("load proposal");
    assert_eq!(proposal.source.source_type, ProposalSourceType::Workflow);
    assert_eq!(
        proposal.source.idempotency_key().as_deref(),
        Some(format!("{execution_id}:create-proposal").as_str())
    );
    // Retry idempotency: same execution + step must not mint a duplicate.
    let deduped = create_proposal(
        root,
        TransactionProposal {
            id: String::new(),
            source: proposal.source.clone(),
            summary: "duplicate".into(),
            commands: proposal.commands.clone(),
            affected_paths: proposal.affected_paths.clone(),
            warnings: vec![],
            created_at: String::new(),
            status: ProposalStatus::Pending,
            resolved_at: None,
            applied_transaction_id: None,
        },
    )
    .expect("dedupe proposal");
    assert_eq!(deduped.id, proposal_id);
    assert_eq!(list_proposal_summaries(root).expect("list proposals").len(), 1);
    log_boundary(
        "proposal",
        &format!("proposal_id={proposal_id} file={}", proposal_path.display()),
    );

    // 5. Proposal preview resolves.
    let preview = preview_proposal(root, &proposal, &[0]).expect("preview proposal");
    assert_eq!(preview.proposal_id, proposal_id);
    assert!(preview.subset_valid);
    assert!(preview.subset_errors.is_empty());
    assert_eq!(preview.commands.len(), 1);
    log_boundary("proposal-preview", "subset_valid=true");

    // 6–7. Selected commands apply; expected page resource exists.
    let history_before_apply = history_transaction_count(root);
    apply_proposal(root, &proposal_id, &[0]).expect("apply proposal");
    assert!(
        await_until("proposal-page", || root.join(PROPOSAL_PAGE).is_file(), 64),
        "proposal page not materialized"
    );
    assert_eq!(history_transaction_count(root), history_before_apply + 1);
    let apply_tx = latest_history_id(root);
    assert!(
        !root.join(".lattice/proposals").join(format!("{proposal_id}.json")).is_file(),
        "proposal file should be dismissed after apply"
    );
    log_boundary(
        "proposal-apply",
        &format!("transaction_id={apply_tx} resource={PROPOSAL_PAGE}"),
    );

    // 8. Derived stale → current after rebuild (T4 APIs).
    let derived_rel = DERIVED_RESOURCE;
    let initial = load_derived_status(root, derived_rel).expect("derived status");
    assert_eq!(initial.state, DerivedState::Stale);
    assert!(initial
        .stale_reasons
        .contains(&DerivedStaleReason::NeverBuilt));

    let root_for_rebuild = root.to_path_buf();
    let path_for_runner = std::env::var("PATH").expect("PATH with shims");
    let rebuild_handle = thread::spawn(move || {
        let runner = TaskRunner::with_env(EnvProvider::with_path(path_for_runner));
        rebuild_derived(&root_for_rebuild, derived_rel, &runner)
    });
    let mut first_build_id = None;
    assert!(
        await_until("derived-building", || {
            if let Ok(Some(lineage)) = load_lineage(root, derived_rel) {
                if lineage.state == DerivedState::Building {
                    first_build_id = lineage.active_build_id.clone();
                    return first_build_id.is_some();
                }
            }
            false
        }, 10_000),
        "derived rebuild never entered building state"
    );
    let built = rebuild_handle.join().expect("join rebuild thread").expect("initial derived rebuild");
    assert_eq!(built.state, DerivedState::Current);
    assert!(root.join(DERIVED_OUTPUT).is_file());
    assert!(lineage_path(root, derived_rel).is_file());
    let first_build_id = first_build_id.expect("derived build id");
    log_boundary(
        "derived-rebuild",
        &format!("build_id={first_build_id} output={DERIVED_OUTPUT}"),
    );

    fs::write(root.join("Derived/input.txt"), "stale-input\n").expect("touch derived input");
    let stale = load_derived_status(root, derived_rel).expect("derived stale status");
    assert_eq!(stale.state, DerivedState::Stale);
    assert!(stale
        .stale_reasons
        .contains(&DerivedStaleReason::InputChanged));
    let prior_hash = stale.output_hash.clone();

    let root_for_refresh = root.to_path_buf();
    let path_for_runner = std::env::var("PATH").expect("PATH with shims");
    let refresh_handle = thread::spawn(move || {
        let runner = TaskRunner::with_env(EnvProvider::with_path(path_for_runner));
        rebuild_derived(&root_for_refresh, derived_rel, &runner)
    });
    let mut second_build_id = None;
    assert!(
        await_until("derived-refresh-building", || {
            if let Ok(Some(lineage)) = load_lineage(root, derived_rel) {
                if lineage.state == DerivedState::Building {
                    second_build_id = lineage.active_build_id.clone();
                    return second_build_id.is_some();
                }
            }
            false
        }, 10_000),
        "derived refresh never entered building state"
    );
    let refreshed = refresh_handle
        .join()
        .expect("join refresh thread")
        .expect("refresh derived");
    assert_eq!(refreshed.state, DerivedState::Current);
    let second_build_id = second_build_id.expect("refresh build id");
    assert_ne!(first_build_id, second_build_id);
    assert_ne!(refreshed.output_hash, prior_hash);
    log_boundary(
        "derived-refresh",
        &format!("build_id={second_build_id} stale→current"),
    );

    // 9. Relationship edge from workflow to CRM package / task.
    let edges = list_relationship_edges(
        root,
        Some(INTAKE_WORKFLOW),
        Some(&[RelationshipKind::Workflow]),
    )
    .expect("relationship edges");
    assert!(
        edges.iter().any(|edge| edge.to == CRM_PACKAGE),
        "missing workflow→CRM.data edge: {edges:?}"
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge.to.contains("ContactIntakeHello")),
        "missing workflow→task edge: {edges:?}"
    );
    log_boundary("relationships", &format!("{} workflow edges", edges.len()));

    // 10. Undo restores prior state (proposal page removed).
    let mut engine = CommandEngine::open(root).expect("open engine for undo");
    let undone = engine.undo().expect("undo").expect("undo receipt");
    assert_eq!(undone.transaction_id, apply_tx);
    assert!(
        await_until("undo-page", || !root.join(PROPOSAL_PAGE).exists(), 64),
        "proposal page still present after undo"
    );
    log_boundary(
        "undo",
        &format!("transaction_id={} restored", undone.transaction_id),
    );

    // Final sanity: insert transaction still in history; workflow + run artifacts remain.
    assert!(run_path.is_file());
    assert!(history_transaction_count(root) >= 2);
}
