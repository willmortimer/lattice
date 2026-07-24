//! Interval schedule runner for open sessions and registered closed-desktop roots.
//!
//! Discovers enabled `type: schedule` workflows, evaluates interval due times,
//! and executes them via [`lattice_commands::load_and_run_workflow`] with
//! trigger label `schedule`. Runs persist under `.lattice/workflows/runs/`.
//!
//! Cron-only schedules are skipped with a debug log until a cron evaluator lands
//! (see [`lattice_commands::ScheduleDue::CronDeferred`]). Desktop event triggers
//! (`resource.changed` / `form.submitted`) are unchanged.
//!
//! **Honest lifecycle (bounded V1):**
//! - Interval schedules fire for warm UI sessions **and** for workspaces the
//!   user registered for background schedules (known-workspace registry).
//! - When any registered workspace is enabled with `keepRunning`, the daemon
//!   holds a scheduler lease so idle shutdown does not stop `latticed`.
//! - Registered roots are opened on demand for due work, then released when the
//!   tick did not inherit an already-warm UI session.
//! - Cron evaluation and durable offline job queues remain deferred.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lattice_commands::{
    discover_scheduled_workflows, evaluate_schedule_due, last_schedule_run_at,
    load_and_run_workflow, proposal_now_iso, ScheduleDue,
};
use lattice_profile::{default_scheduler_registry_path, KnownWorkspaceRegistry};
use lattice_runtime::LatticeRuntime;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::idle::ConnectionTracker;
use crate::jobs::{
    reconcile_or_warn, running_schedule_summary, summary_from_finished, JobRegistry,
};

/// Default polling period for schedule evaluation.
pub const DEFAULT_SCHEDULE_TICK: Duration = Duration::from_secs(5);

/// In-process schedule runner over warm and on-demand [`LatticeRuntime`] sessions.
pub struct ScheduleRunner {
    runtime: Arc<LatticeRuntime>,
    jobs: Arc<JobRegistry>,
    connections: Option<Arc<ConnectionTracker>>,
    registry_path: PathBuf,
    /// Last fire time keyed by `(workspace_root, workflow_rel_path)`.
    last_fire: HashMap<(PathBuf, String), SystemTime>,
    /// Workflows currently executing a schedule-sourced run.
    in_flight: HashSet<(PathBuf, String)>,
    /// Missing-root roots we already warned about (avoid tight-loop log spam).
    missing_root_warned: HashSet<PathBuf>,
}

impl ScheduleRunner {
    pub fn new(runtime: Arc<LatticeRuntime>, jobs: Arc<JobRegistry>) -> Self {
        Self::with_registry_path(runtime, jobs, None, default_scheduler_registry_path())
    }

    pub fn with_connections(
        runtime: Arc<LatticeRuntime>,
        jobs: Arc<JobRegistry>,
        connections: Arc<ConnectionTracker>,
    ) -> Self {
        Self::with_registry_path(
            runtime,
            jobs,
            Some(connections),
            default_scheduler_registry_path(),
        )
    }

    pub fn with_registry_path(
        runtime: Arc<LatticeRuntime>,
        jobs: Arc<JobRegistry>,
        connections: Option<Arc<ConnectionTracker>>,
        registry_path: PathBuf,
    ) -> Self {
        Self {
            runtime,
            jobs,
            connections,
            registry_path,
            last_fire: HashMap::new(),
            in_flight: HashSet::new(),
            missing_root_warned: HashSet::new(),
        }
    }

    fn load_registry(&self) -> KnownWorkspaceRegistry {
        KnownWorkspaceRegistry::load_or_default(&self.registry_path).unwrap_or_default()
    }

    fn save_registry(&self, registry: &KnownWorkspaceRegistry) {
        if let Err(err) = registry.save(&self.registry_path) {
            warn!(
                path = %self.registry_path.display(),
                error = %err,
                "failed to persist known-workspace registry"
            );
        }
    }

    async fn sync_scheduler_lease(&self, registry: &KnownWorkspaceRegistry) {
        let Some(connections) = self.connections.as_ref() else {
            return;
        };
        connections
            .set_scheduler_lease(registry.scheduler_lease_active())
            .await;
    }

    /// Evaluate open sessions and registered closed-desktop roots once.
    pub async fn tick_once(&mut self) {
        let mut registry = self.load_registry();
        self.sync_scheduler_lease(&registry).await;

        let warm_roots: HashSet<PathBuf> = self.runtime.list_session_roots().into_iter().collect();
        let mut touched: HashSet<PathBuf> = HashSet::new();

        for root in warm_roots.iter() {
            reconcile_or_warn(&self.jobs, root);
            self.tick_workspace(root).await;
            touched.insert(root.clone());
            self.record_successful_scan(&mut registry, root).await;
        }

        let enabled: Vec<(PathBuf, bool)> = registry
            .enabled_entries()
            .map(|entry| (PathBuf::from(&entry.root), entry.keep_running))
            .collect();

        for (root, _keep_running) in enabled {
            if touched.contains(&root) {
                continue;
            }
            if !root.is_dir() {
                self.record_missing_root(&mut registry, &root);
                continue;
            }
            self.missing_root_warned.remove(&root);

            let opened_on_demand = match self.runtime.get_session(&root) {
                Ok(Some(_)) => false,
                Ok(None) => match self.runtime.open_workspace_session(&root) {
                    Ok(_) => true,
                    Err(err) => {
                        self.record_open_failure(&mut registry, &root, &err.to_string());
                        continue;
                    }
                },
                Err(err) => {
                    self.record_open_failure(&mut registry, &root, &err.to_string());
                    continue;
                }
            };

            reconcile_or_warn(&self.jobs, &root);
            self.tick_workspace(&root).await;
            self.record_successful_scan(&mut registry, &root).await;
            touched.insert(root.clone());

            if opened_on_demand {
                if let Err(err) = self.runtime.close_session(&root) {
                    warn!(
                        root = %root.display(),
                        error = %err,
                        "failed to release on-demand schedule session"
                    );
                }
            }
        }

        self.save_registry(&registry);
        self.sync_scheduler_lease(&registry).await;
    }

    async fn record_successful_scan(&self, registry: &mut KnownWorkspaceRegistry, root: &Path) {
        let workflows = match tokio::task::spawn_blocking({
            let root = root.to_path_buf();
            move || discover_scheduled_workflows(&root)
        })
        .await
        {
            Ok(Ok(items)) => items
                .iter()
                .map(|item| item.relative_path(root))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };

        let workspace_id = self
            .runtime
            .get_session(root)
            .ok()
            .flatten()
            .map(|session| session.workspace_id().to_string());

        if let Some(entry) = registry.get_mut(root) {
            entry.schedule_workflows = workflows;
            entry.last_scan_at = Some(proposal_now_iso());
            entry.last_error = None;
            if let Some(id) = workspace_id {
                entry.workspace_id = Some(id);
            }
        }
    }

    fn record_missing_root(&mut self, registry: &mut KnownWorkspaceRegistry, root: &Path) {
        let message = format!(
            "workspace root missing or not a directory: {}",
            root.display()
        );
        if let Some(entry) = registry.get_mut(root) {
            entry.last_error = Some(message.clone());
            entry.last_scan_at = Some(proposal_now_iso());
        }
        if self.missing_root_warned.insert(root.to_path_buf()) {
            warn!(root = %root.display(), "registered schedule workspace root missing");
        } else {
            debug!(root = %root.display(), "registered schedule workspace root still missing");
        }
    }

    fn record_open_failure(
        &mut self,
        registry: &mut KnownWorkspaceRegistry,
        root: &Path,
        detail: &str,
    ) {
        let message = format!("failed to open workspace for schedule tick: {detail}");
        if let Some(entry) = registry.get_mut(root) {
            entry.last_error = Some(message.clone());
            entry.last_scan_at = Some(proposal_now_iso());
        }
        warn!(root = %root.display(), error = %detail, "schedule on-demand open failed");
    }

    /// Evaluate schedule triggers for a single workspace root.
    pub async fn tick_workspace(&mut self, workspace_root: &Path) {
        let root = workspace_root.to_path_buf();
        let scheduled = match tokio::task::spawn_blocking({
            let root = root.clone();
            move || discover_scheduled_workflows(&root)
        })
        .await
        {
            Ok(Ok(items)) => items,
            Ok(Err(err)) => {
                warn!(
                    root = %root.display(),
                    error = %err,
                    "schedule discovery failed"
                );
                return;
            }
            Err(err) => {
                warn!(
                    root = %root.display(),
                    error = %err,
                    "schedule discovery task join failed"
                );
                return;
            }
        };

        let now = SystemTime::now();
        for item in scheduled {
            let rel = item.relative_path(&root);
            let key = (root.clone(), rel.clone());
            if self.in_flight.contains(&key) {
                continue;
            }

            let disk_last = match tokio::task::spawn_blocking({
                let root = root.clone();
                let rel = rel.clone();
                move || last_schedule_run_at(&root, &rel)
            })
            .await
            {
                Ok(Ok(value)) => value,
                Ok(Err(err)) => {
                    warn!(
                        workflow = %rel,
                        error = %err,
                        "failed to read schedule run history"
                    );
                    None
                }
                Err(err) => {
                    warn!(
                        workflow = %rel,
                        error = %err,
                        "schedule history task join failed"
                    );
                    None
                }
            };

            let mem_last = self.last_fire.get(&key).copied();
            let last_fire = match (disk_last, mem_last) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };

            match evaluate_schedule_due(item.schedule(), last_fire, now) {
                ScheduleDue::NotDue => {}
                ScheduleDue::CronDeferred => {
                    debug!(
                        workflow = %rel,
                        "cron-only schedule deferred (no cron evaluator yet; set interval_seconds to fire)"
                    );
                }
                ScheduleDue::Due => {
                    self.in_flight.insert(key.clone());
                    let workflow_path = item.path.clone();
                    let run_root = root.clone();
                    let fired_at = SystemTime::now();
                    let execution_id = uuid::Uuid::now_v7().to_string();
                    let started_at = proposal_now_iso();
                    let cancel = Arc::new(AtomicBool::new(false));
                    let summary =
                        running_schedule_summary(&run_root, &rel, &execution_id, &started_at);
                    if let Err(err) = self.jobs.begin(summary, Arc::clone(&cancel)) {
                        warn!(
                            workflow = %rel,
                            error = %err,
                            "failed to register schedule job"
                        );
                    }

                    let result = tokio::task::spawn_blocking({
                        let cancel = Arc::clone(&cancel);
                        let execution_id = execution_id.clone();
                        move || {
                            load_and_run_workflow(
                                &run_root,
                                &workflow_path,
                                Some("schedule"),
                                Some(cancel.as_ref()),
                                Some(execution_id.as_str()),
                            )
                        }
                    })
                    .await;
                    self.in_flight.remove(&key);
                    match result {
                        Ok(Ok(record)) => {
                            self.last_fire.insert(key, fired_at);
                            let finished = summary_from_finished(&root, &record);
                            let _ = self.jobs.finish(finished);
                            info!(
                                workflow = %rel,
                                execution_id = %record.execution.id,
                                status = ?record.execution.status,
                                "schedule workflow fired"
                            );
                        }
                        Ok(Err(err)) => {
                            // Still advance last_fire so a hard failure does not tight-loop.
                            self.last_fire.insert(key, fired_at);
                            let failed = lattice_commands::ExecutionSummary {
                                execution_id: execution_id.clone(),
                                workspace_root: root.to_string_lossy().replace('\\', "/"),
                                resource_path: rel.clone(),
                                kind: "workflow".into(),
                                trigger: "schedule".into(),
                                status: lattice_commands::ExecutionStatus::Failed,
                                started_at,
                                finished_at: Some(proposal_now_iso()),
                                current_step_id: None,
                                attempt: None,
                                proposal_ids: Vec::new(),
                                cancel_owner: lattice_commands::CANCEL_OWNER_NONE.into(),
                                cancellable: false,
                            };
                            let _ = self.jobs.finish(failed);
                            warn!(
                                workflow = %rel,
                                error = %err,
                                "schedule workflow run failed"
                            );
                        }
                        Err(err) => {
                            let failed = lattice_commands::ExecutionSummary {
                                execution_id: execution_id.clone(),
                                workspace_root: root.to_string_lossy().replace('\\', "/"),
                                resource_path: rel.clone(),
                                kind: "workflow".into(),
                                trigger: "schedule".into(),
                                status: lattice_commands::ExecutionStatus::Failed,
                                started_at,
                                finished_at: Some(proposal_now_iso()),
                                current_step_id: None,
                                attempt: None,
                                proposal_ids: Vec::new(),
                                cancel_owner: lattice_commands::CANCEL_OWNER_NONE.into(),
                                cancellable: false,
                            };
                            let _ = self.jobs.finish(failed);
                            warn!(
                                workflow = %rel,
                                error = %err,
                                "schedule workflow task join failed"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Spawn a background loop that ticks open + registered workspaces every `tick`.
///
/// Abort the returned handle on daemon shutdown.
pub fn spawn_schedule_runner(
    runtime: Arc<LatticeRuntime>,
    jobs: Arc<JobRegistry>,
    tick: Duration,
) -> JoinHandle<()> {
    spawn_schedule_runner_with_connections(runtime, jobs, None, tick)
}

/// Spawn the schedule runner with an optional connection tracker for leases.
pub fn spawn_schedule_runner_with_connections(
    runtime: Arc<LatticeRuntime>,
    jobs: Arc<JobRegistry>,
    connections: Option<Arc<ConnectionTracker>>,
    tick: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut runner = match connections {
            Some(connections) => ScheduleRunner::with_connections(runtime, jobs, connections),
            None => ScheduleRunner::new(runtime, jobs),
        };
        let mut interval = tokio::time::interval(tick);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // First tick completes immediately; skip so we wait one period after start.
        interval.tick().await;
        info!(
            secs = tick.as_secs_f64(),
            "schedule runner started (interval on open sessions + registered closed-desktop roots; cron still deferred)"
        );
        loop {
            interval.tick().await;
            runner.tick_once().await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    use lattice_commands::{list_workflow_runs, WorkflowTrigger};
    use lattice_core::Workspace;
    use lattice_profile::LATTICE_SCHEDULER_REGISTRY_ENV;
    use tempfile::TempDir;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn init_workspace() -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        Workspace::init(dir.path(), "Schedule Runner Test").expect("init");
        dir
    }

    fn write_interval_workflow(root: &Path, name: &str, enabled: bool, interval_seconds: u64) {
        let path = root.join(format!("{name}.workflow.yaml"));
        let enabled_yaml = if enabled { "true" } else { "false" };
        fs::write(
            path,
            format!(
                r#"
format: lattice-workflow
version: 1
name: {name}
enabled: {enabled_yaml}
trigger:
  type: schedule
  interval_seconds: {interval_seconds}
steps:
  - id: note
    action: notification
    with:
      message: schedule tick from {name}
"#
            ),
        )
        .expect("write workflow");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fires_due_interval_workflow_and_skips_disabled() {
        let dir = init_workspace();
        let root = dir.path();
        write_interval_workflow(root, "Enabled", true, 1);
        write_interval_workflow(root, "Disabled", false, 1);

        let runtime = Arc::new(LatticeRuntime::new());
        let jobs = Arc::new(JobRegistry::new());
        let _session = runtime.open_workspace_session(root).expect("open session");

        let mut runner = ScheduleRunner::new(Arc::clone(&runtime), Arc::clone(&jobs));
        runner.tick_once().await;

        let enabled_runs =
            list_workflow_runs(root, "Enabled.workflow.yaml", 8).expect("enabled runs");
        assert_eq!(enabled_runs.len(), 1);
        assert_eq!(enabled_runs[0].trigger, "schedule");
        assert!(matches!(
            load_trigger(root, "Enabled.workflow.yaml"),
            WorkflowTrigger::Schedule(_)
        ));

        let active = jobs.list_active().expect("active");
        assert!(
            active.is_empty(),
            "completed schedule run should leave active empty"
        );
        let recent = jobs.list_recent(8).expect("recent");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].execution_id, enabled_runs[0].execution.id);
        assert_eq!(recent[0].trigger, "schedule");

        let disabled_runs =
            list_workflow_runs(root, "Disabled.workflow.yaml", 8).expect("disabled runs");
        assert!(disabled_runs.is_empty());

        // Immediate second tick should not re-fire (interval not elapsed).
        runner.tick_once().await;
        let enabled_runs =
            list_workflow_runs(root, "Enabled.workflow.yaml", 8).expect("enabled runs");
        assert_eq!(enabled_runs.len(), 1);

        tokio::time::sleep(Duration::from_millis(1_100)).await;
        runner.tick_once().await;
        let enabled_runs =
            list_workflow_runs(root, "Enabled.workflow.yaml", 8).expect("enabled runs");
        assert_eq!(enabled_runs.len(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fires_registered_workspace_without_open_ui_session() {
        let _guard = env_lock();
        let dir = init_workspace();
        let root = dir.path();
        write_interval_workflow(root, "Background", true, 1);

        let registry_dir = tempfile::tempdir().expect("registry dir");
        let registry_path = registry_dir.path().join("workspaces.json");
        std::env::set_var(LATTICE_SCHEDULER_REGISTRY_ENV, &registry_path);

        let mut registry = KnownWorkspaceRegistry::default();
        registry.register(root, true);
        registry.save(&registry_path).expect("save registry");

        let runtime = Arc::new(LatticeRuntime::new());
        let jobs = Arc::new(JobRegistry::new());
        assert_eq!(runtime.session_count(), 0);

        let mut runner = ScheduleRunner::with_registry_path(
            Arc::clone(&runtime),
            Arc::clone(&jobs),
            None,
            registry_path.clone(),
        );
        runner.tick_once().await;

        let runs = list_workflow_runs(root, "Background.workflow.yaml", 8).expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].trigger, "schedule");
        assert_eq!(
            runtime.session_count(),
            0,
            "on-demand session should be released after tick"
        );

        let loaded = KnownWorkspaceRegistry::load_or_default(&registry_path).expect("reload");
        let entry = loaded.get(root).expect("registered");
        assert!(entry.last_error.is_none());
        assert!(entry
            .schedule_workflows
            .iter()
            .any(|path| path == "Background.workflow.yaml"));

        std::env::remove_var(LATTICE_SCHEDULER_REGISTRY_ENV);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_root_records_failed_schedule_state_without_tight_loop() {
        let registry_dir = tempfile::tempdir().expect("registry dir");
        let registry_path = registry_dir.path().join("workspaces.json");
        let missing = registry_dir.path().join("gone-workspace");

        let mut registry = KnownWorkspaceRegistry::default();
        registry.register(&missing, true);
        // Force a non-canonical path that will not exist.
        registry.workspaces[0].root = missing.to_string_lossy().replace('\\', "/");
        registry.save(&registry_path).expect("save");

        let runtime = Arc::new(LatticeRuntime::new());
        let jobs = Arc::new(JobRegistry::new());
        let mut runner = ScheduleRunner::with_registry_path(
            Arc::clone(&runtime),
            Arc::clone(&jobs),
            None,
            registry_path.clone(),
        );
        runner.tick_once().await;
        runner.tick_once().await;

        let loaded = KnownWorkspaceRegistry::load_or_default(&registry_path).expect("reload");
        let entry = &loaded.workspaces[0];
        let err = entry.last_error.as_deref().expect("last_error");
        assert!(
            err.contains("missing") || err.contains("not a directory"),
            "unexpected last_error: {err}"
        );
        assert_eq!(runtime.session_count(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn registry_lease_keeps_connection_tracker_intent() {
        let dir = init_workspace();
        let root = dir.path();
        let registry_dir = tempfile::tempdir().expect("registry dir");
        let registry_path = registry_dir.path().join("workspaces.json");

        let mut registry = KnownWorkspaceRegistry::default();
        registry.register(root, true);
        registry.save(&registry_path).expect("save");

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let tracker = ConnectionTracker::new(false, Duration::from_millis(40), tx);
        let runtime = Arc::new(LatticeRuntime::new());
        let jobs = Arc::new(JobRegistry::new());
        let mut runner = ScheduleRunner::with_registry_path(
            Arc::clone(&runtime),
            Arc::clone(&jobs),
            Some(Arc::clone(&tracker)),
            registry_path,
        );
        runner.tick_once().await;
        assert!(tracker.scheduler_lease_held());

        tracker.on_connect().await;
        drop(tracker.guard());
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(rx.try_recv().is_err(), "lease should block idle shutdown");
    }

    fn load_trigger(root: &Path, rel: &str) -> WorkflowTrigger {
        use lattice_commands::WorkflowManifest;
        WorkflowManifest::load(&root.join(rel))
            .expect("load")
            .trigger
    }
}
