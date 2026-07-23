//! Interval schedule runner skeleton for open workspace sessions.
//!
//! Discovers enabled `type: schedule` workflows, evaluates interval due times,
//! and executes them via [`lattice_commands::load_and_run_workflow`] with trigger
//! label `schedule`. Runs persist under `.lattice/workflows/runs/`.
//!
//! Cron-only schedules are skipped with a debug log until a cron evaluator lands
//! (see [`lattice_commands::ScheduleDue::CronDeferred`]). Desktop event triggers
//! (`resource.changed` / `form.submitted`) are unchanged.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lattice_commands::{
    discover_scheduled_workflows, evaluate_schedule_due, last_schedule_run_at,
    load_and_run_workflow, ScheduleDue,
};
use lattice_runtime::LatticeRuntime;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Default polling period for open-session schedule evaluation.
pub const DEFAULT_SCHEDULE_TICK: Duration = Duration::from_secs(5);

/// In-process schedule runner over warm [`LatticeRuntime`] sessions.
#[derive(Debug)]
pub struct ScheduleRunner {
    runtime: Arc<LatticeRuntime>,
    /// Last fire time keyed by `(workspace_root, workflow_rel_path)`.
    last_fire: HashMap<(PathBuf, String), SystemTime>,
    /// Workflows currently executing a schedule-sourced run.
    in_flight: HashSet<(PathBuf, String)>,
}

impl ScheduleRunner {
    pub fn new(runtime: Arc<LatticeRuntime>) -> Self {
        Self {
            runtime,
            last_fire: HashMap::new(),
            in_flight: HashSet::new(),
        }
    }

    /// Evaluate every open session once (serial per due workflow).
    pub async fn tick_once(&mut self) {
        let roots = self.runtime.list_session_roots();
        for root in roots {
            self.tick_workspace(&root).await;
        }
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
                    let result = tokio::task::spawn_blocking(move || {
                        load_and_run_workflow(&run_root, &workflow_path, Some("schedule"), None)
                    })
                    .await;
                    self.in_flight.remove(&key);
                    match result {
                        Ok(Ok(record)) => {
                            self.last_fire.insert(key, fired_at);
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
                            warn!(
                                workflow = %rel,
                                error = %err,
                                "schedule workflow run failed"
                            );
                        }
                        Err(err) => {
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

/// Spawn a background loop that ticks open workspaces every `tick`.
///
/// Abort the returned handle on daemon shutdown.
pub fn spawn_schedule_runner(
    runtime: Arc<LatticeRuntime>,
    tick: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut runner = ScheduleRunner::new(runtime);
        let mut interval = tokio::time::interval(tick);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // First tick completes immediately; skip so we wait one period after start.
        interval.tick().await;
        info!(
            secs = tick.as_secs_f64(),
            "schedule runner started (interval schedules on open sessions)"
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

    use lattice_commands::{list_workflow_runs, WorkflowTrigger};
    use lattice_core::Workspace;
    use tempfile::TempDir;

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
        let _session = runtime
            .open_workspace_session(root)
            .expect("open session");

        let mut runner = ScheduleRunner::new(Arc::clone(&runtime));
        runner.tick_once().await;

        let enabled_runs =
            list_workflow_runs(root, "Enabled.workflow.yaml", 8).expect("enabled runs");
        assert_eq!(enabled_runs.len(), 1);
        assert_eq!(enabled_runs[0].trigger, "schedule");
        assert!(matches!(
            load_trigger(root, "Enabled.workflow.yaml"),
            WorkflowTrigger::Schedule(_)
        ));

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

    fn load_trigger(root: &Path, rel: &str) -> WorkflowTrigger {
        use lattice_commands::WorkflowManifest;
        WorkflowManifest::load(&root.join(rel))
            .expect("load")
            .trigger
    }
}
