//! In-memory job registry for daemon-owned workflow executions.
//!
//! Schedule runner and HTTP job APIs share this registry so tray/desktop can
//! list active runs and cancel with a single owner (`cancel_owner: daemon`).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lattice_commands::{
    execution_summary_from_record, list_all_workflow_runs, load_workflow_run,
    reconcile_abandoned_workflow_runs, ExecutionStatus, ExecutionSummary, WorkflowRunRecord,
    CANCEL_OWNER_DAEMON, CANCEL_OWNER_NONE,
};
use tracing::{info, warn};

use crate::api::ApiError;

const RECENT_CAP: usize = 64;

struct LiveJob {
    summary: ExecutionSummary,
    cancel: Arc<AtomicBool>,
}

/// Shared registry of daemon-owned executions.
#[derive(Default)]
pub struct JobRegistry {
    active: Mutex<HashMap<String, LiveJob>>,
    recent: Mutex<VecDeque<ExecutionSummary>>,
    /// Workspace roots already reconciled for abandoned runs this process.
    reconciled: Mutex<HashMap<PathBuf, ()>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a newly started daemon-owned run.
    pub fn begin(
        &self,
        summary: ExecutionSummary,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), ApiError> {
        let id = summary.execution_id.clone();
        let mut map = self
            .active
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        map.insert(id, LiveJob { summary, cancel });
        Ok(())
    }

    /// Finish a run: drop active entry and push onto recent history.
    pub fn finish(&self, summary: ExecutionSummary) -> Result<(), ApiError> {
        let id = summary.execution_id.clone();
        {
            let mut map = self
                .active
                .lock()
                .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
            map.remove(&id);
        }
        let mut recent = self
            .recent
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        recent.retain(|item| item.execution_id != id);
        recent.push_front(summary);
        while recent.len() > RECENT_CAP {
            recent.pop_back();
        }
        Ok(())
    }

    /// Update the live summary for an in-flight job (best-effort).
    pub fn update_summary(&self, summary: ExecutionSummary) -> Result<(), ApiError> {
        let mut map = self
            .active
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        if let Some(live) = map.get_mut(&summary.execution_id) {
            live.summary = summary;
        }
        Ok(())
    }

    pub fn list_active(&self) -> Result<Vec<ExecutionSummary>, ApiError> {
        let map = self
            .active
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        let mut jobs: Vec<_> = map.values().map(|live| live.summary.clone()).collect();
        jobs.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| right.execution_id.cmp(&left.execution_id))
        });
        Ok(jobs)
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<ExecutionSummary>, ApiError> {
        let limit = limit.clamp(1, RECENT_CAP);
        let mut combined = self.list_active()?;
        let recent = self
            .recent
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        for item in recent.iter() {
            if combined
                .iter()
                .any(|existing| existing.execution_id == item.execution_id)
            {
                continue;
            }
            combined.push(item.clone());
        }
        combined.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| right.execution_id.cmp(&left.execution_id))
        });
        combined.truncate(limit);
        Ok(combined)
    }

    pub fn get(&self, execution_id: &str) -> Result<ExecutionSummary, ApiError> {
        {
            let map = self
                .active
                .lock()
                .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
            if let Some(live) = map.get(execution_id) {
                return Ok(live.summary.clone());
            }
        }
        let recent = self
            .recent
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        recent
            .iter()
            .find(|item| item.execution_id == execution_id)
            .cloned()
            .ok_or_else(|| ApiError::NotFound(format!("job not found: {execution_id}")))
    }

    /// Request cooperative cancel for a daemon-owned active job.
    pub fn cancel(&self, execution_id: &str) -> Result<ExecutionSummary, ApiError> {
        let map = self
            .active
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        let live = map.get(execution_id).ok_or_else(|| {
            ApiError::NotFound(format!(
                "active job not found (cannot cancel): {execution_id}"
            ))
        })?;
        if !live.summary.cancellable || live.summary.cancel_owner != CANCEL_OWNER_DAEMON {
            return Err(ApiError::BadRequest(format!(
                "job {execution_id} is not cancellable by the daemon"
            )));
        }
        live.cancel.store(true, Ordering::SeqCst);
        Ok(live.summary.clone())
    }

    /// Reconcile abandoned on-disk runs once per workspace root for this process.
    pub fn reconcile_workspace(&self, workspace_root: &Path) -> Result<usize, ApiError> {
        let key = workspace_root.to_path_buf();
        {
            let reconciled = self
                .reconciled
                .lock()
                .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
            if reconciled.contains_key(&key) {
                return Ok(0);
            }
        }
        let marked = reconcile_abandoned_workflow_runs(workspace_root)
            .map_err(|err| ApiError::Internal(err.to_string()))?;
        if marked > 0 {
            info!(
                root = %workspace_root.display(),
                marked,
                "marked abandoned workflow runs after daemon start"
            );
        }
        let mut reconciled = self
            .reconciled
            .lock()
            .map_err(|_| ApiError::Internal("job registry lock poisoned".into()))?;
        reconciled.insert(key, ());
        Ok(marked)
    }
}

/// Build a running summary for a schedule-sourced workflow.
pub fn running_schedule_summary(
    workspace_root: &Path,
    workflow_rel: &str,
    execution_id: &str,
    started_at: &str,
) -> ExecutionSummary {
    ExecutionSummary {
        execution_id: execution_id.to_string(),
        workspace_root: workspace_root.to_string_lossy().replace('\\', "/"),
        resource_path: workflow_rel.to_string(),
        kind: "workflow".into(),
        trigger: "schedule".into(),
        status: ExecutionStatus::Running,
        started_at: started_at.to_string(),
        finished_at: None,
        current_step_id: None,
        attempt: None,
        proposal_ids: Vec::new(),
        cancel_owner: CANCEL_OWNER_DAEMON.into(),
        cancellable: true,
    }
}

pub fn summary_from_finished(
    workspace_root: &Path,
    record: &WorkflowRunRecord,
) -> ExecutionSummary {
    let mut summary = execution_summary_from_record(workspace_root, record, CANCEL_OWNER_DAEMON);
    // Terminal runs are not cancellable.
    if summary.status != ExecutionStatus::Running {
        summary.cancel_owner = CANCEL_OWNER_NONE.into();
        summary.cancellable = false;
    }
    summary
}

/// Merge in-memory recent with on-disk history for a workspace (HTTP detail helper).
pub fn disk_recent_for_workspace(
    workspace_root: &Path,
    limit: usize,
) -> Result<Vec<ExecutionSummary>, ApiError> {
    let runs = list_all_workflow_runs(workspace_root, limit)
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    Ok(runs
        .iter()
        .map(|record| {
            let owner = if record.execution.status == ExecutionStatus::Running {
                CANCEL_OWNER_DAEMON
            } else {
                CANCEL_OWNER_NONE
            };
            let mut summary = execution_summary_from_record(workspace_root, record, owner);
            if summary.status != ExecutionStatus::Running {
                summary.cancellable = false;
                summary.cancel_owner = CANCEL_OWNER_NONE.into();
            }
            summary
        })
        .collect())
}

pub fn disk_get(workspace_root: &Path, execution_id: &str) -> Result<ExecutionSummary, ApiError> {
    let record = load_workflow_run(workspace_root, execution_id).map_err(|err| {
        if matches!(err, lattice_commands::WorkflowError::Io { .. }) {
            ApiError::NotFound(format!("job not found: {execution_id}"))
        } else {
            ApiError::Internal(err.to_string())
        }
    })?;
    let owner = if record.execution.status == ExecutionStatus::Running {
        CANCEL_OWNER_DAEMON
    } else {
        CANCEL_OWNER_NONE
    };
    let mut summary = execution_summary_from_record(workspace_root, &record, owner);
    if summary.status != ExecutionStatus::Running {
        summary.cancellable = false;
        summary.cancel_owner = CANCEL_OWNER_NONE.into();
    }
    Ok(summary)
}

/// Best-effort warn wrapper used from OpenWorkspace.
pub fn reconcile_or_warn(jobs: &JobRegistry, workspace_root: &Path) {
    if let Err(err) = jobs.reconcile_workspace(workspace_root) {
        warn!(
            root = %workspace_root.display(),
            error = %err,
            "failed to reconcile abandoned workflow runs"
        );
    }
}
