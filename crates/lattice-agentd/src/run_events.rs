//! KernelFS run lifecycle events via latticed `POST /v1/agent_runs/{id}/events`.
//!
//! Typed monotonic stages (`run.created` … `run.released`) for WASI and Cell
//! execution runs. Emission is best-effort: failures are logged and do not fail
//! the guest run.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::lattice_client::{LatticeApiError, LatticeToolClient};

/// KernelFS lifecycle event type strings (dot-separated).
pub const EVENT_RUN_CREATED: &str = "run.created";
pub const EVENT_RUN_HYDRATING: &str = "run.hydrating";
pub const EVENT_RUN_READY: &str = "run.ready";
pub const EVENT_RUN_EXECUTING: &str = "run.executing";
pub const EVENT_RUN_OUTPUT_AVAILABLE: &str = "run.output_available";
pub const EVENT_RUN_PROPOSAL_READY: &str = "run.proposal_ready";
pub const EVENT_RUN_FAILED: &str = "run.failed";
pub const EVENT_RUN_RELEASED: &str = "run.released";

/// One lifecycle stage recorded during a blocking WASI host run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelfsLifecycleStage {
    Created,
    Hydrating { input_count: usize },
    Ready,
    Executing,
    OutputAvailable {
        exit_code: i32,
        draft_count: usize,
    },
    Failed {
        kind: String,
        message: String,
    },
}

/// Collect lifecycle stages from a sync WASI host path; flush async after join.
#[derive(Debug, Default)]
pub struct KernelfsLifecycleCollector {
    stages: Mutex<Vec<KernelfsLifecycleStage>>,
}

impl KernelfsLifecycleCollector {
    pub fn hook(self: &Arc<Self>) -> Arc<dyn Fn(KernelfsLifecycleStage) + Send + Sync> {
        let collector = Arc::clone(self);
        Arc::new(move |stage| {
            collector
                .stages
                .lock()
                .expect("lifecycle collector poisoned")
                .push(stage);
        })
    }

    pub fn drain(&self) -> Vec<KernelfsLifecycleStage> {
        std::mem::take(
            &mut *self
                .stages
                .lock()
                .expect("lifecycle collector poisoned"),
        )
    }
}

/// Binding for durable KernelFS run events (distinct from the agent chat run id).
#[derive(Debug, Clone)]
pub struct KernelfsLifecycleContext {
    pub kernelfs_run_id: String,
    pub thread_id: String,
    pub workspace_id: Option<String>,
    pub workspace_root: Option<String>,
    pub base_snapshot_id: String,
    pub backend: &'static str,
}

/// HTTP emitter for KernelFS lifecycle stages.
#[derive(Debug, Clone)]
pub struct KernelfsLifecycleEmitter {
    client: LatticeToolClient,
    ctx: KernelfsLifecycleContext,
}

impl KernelfsLifecycleEmitter {
    pub fn new(client: LatticeToolClient, ctx: KernelfsLifecycleContext) -> Self {
        Self { client, ctx }
    }

    pub fn context(&self) -> &KernelfsLifecycleContext {
        &self.ctx
    }

    pub async fn emit_stage(&self, stage: KernelfsLifecycleStage) {
        match stage {
            KernelfsLifecycleStage::Created => self.created().await,
            KernelfsLifecycleStage::Hydrating { input_count } => {
                self.hydrating(input_count).await
            }
            KernelfsLifecycleStage::Ready => self.ready().await,
            KernelfsLifecycleStage::Executing => self.executing().await,
            KernelfsLifecycleStage::OutputAvailable {
                exit_code,
                draft_count,
            } => self.output_available(exit_code, draft_count).await,
            KernelfsLifecycleStage::Failed { kind, message } => {
                self.failed(&kind, &message).await
            }
        }
    }

    pub async fn flush_stages(&self, stages: Vec<KernelfsLifecycleStage>) {
        for stage in stages {
            self.emit_stage(stage).await;
        }
    }

    pub async fn created(&self) {
        self.emit_best_effort(
            EVENT_RUN_CREATED,
            json!({
                "type": EVENT_RUN_CREATED,
                "runId": self.ctx.kernelfs_run_id,
                "workspaceId": self.ctx.workspace_id,
                "baseSnapshotId": self.ctx.base_snapshot_id,
                "backend": self.ctx.backend,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_CREATED,
        )
        .await;
    }

    pub async fn hydrating(&self, input_count: usize) {
        self.emit_best_effort(
            EVENT_RUN_HYDRATING,
            json!({
                "type": EVENT_RUN_HYDRATING,
                "runId": self.ctx.kernelfs_run_id,
                "inputCount": input_count,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_HYDRATING,
        )
        .await;
    }

    pub async fn ready(&self) {
        self.emit_best_effort(
            EVENT_RUN_READY,
            json!({
                "type": EVENT_RUN_READY,
                "runId": self.ctx.kernelfs_run_id,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_READY,
        )
        .await;
    }

    pub async fn executing(&self) {
        self.emit_best_effort(
            EVENT_RUN_EXECUTING,
            json!({
                "type": EVENT_RUN_EXECUTING,
                "runId": self.ctx.kernelfs_run_id,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_EXECUTING,
        )
        .await;
    }

    pub async fn output_available(&self, exit_code: i32, draft_count: usize) {
        self.emit_best_effort(
            EVENT_RUN_OUTPUT_AVAILABLE,
            json!({
                "type": EVENT_RUN_OUTPUT_AVAILABLE,
                "runId": self.ctx.kernelfs_run_id,
                "exitCode": exit_code,
                "draftCount": draft_count,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_OUTPUT_AVAILABLE,
        )
        .await;
    }

    pub async fn proposal_ready(&self, proposal_count: usize) {
        self.emit_best_effort(
            EVENT_RUN_PROPOSAL_READY,
            json!({
                "type": EVENT_RUN_PROPOSAL_READY,
                "runId": self.ctx.kernelfs_run_id,
                "proposalCount": proposal_count,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_PROPOSAL_READY,
        )
        .await;
    }

    pub async fn failed(&self, kind: &str, message: &str) {
        self.emit_best_effort(
            EVENT_RUN_FAILED,
            json!({
                "type": EVENT_RUN_FAILED,
                "runId": self.ctx.kernelfs_run_id,
                "kind": kind,
                "message": message,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_FAILED,
        )
        .await;
    }

    pub async fn released(&self) {
        self.emit_best_effort(
            EVENT_RUN_RELEASED,
            json!({
                "type": EVENT_RUN_RELEASED,
                "runId": self.ctx.kernelfs_run_id,
                "timestamp": now_ms(),
            }),
            EVENT_RUN_RELEASED,
        )
        .await;
    }

    async fn emit_best_effort(&self, event_type: &str, payload: Value, id_suffix: &str) {
        if let Err(err) = self.emit(event_type, payload, id_suffix).await {
            tracing::warn!(
                target: "lattice_agentd",
                run_id = %self.ctx.kernelfs_run_id,
                event_type = %event_type,
                error = %err,
                "failed to append KernelFS lifecycle event"
            );
        }
    }

    async fn emit(
        &self,
        event_type: &str,
        payload: Value,
        id_suffix: &str,
    ) -> Result<(), LatticeApiError> {
        let mut body = json!({
            "threadId": self.ctx.thread_id,
            "eventType": event_type,
            "payload": payload,
            "id": format!("{}:{}", self.ctx.kernelfs_run_id, id_suffix),
        });
        if let Some(workspace_id) = self
            .ctx
            .workspace_id
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            body["workspaceId"] = Value::String(workspace_id.clone());
        }
        if let Some(root) = self
            .ctx
            .workspace_root
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            body["root"] = Value::String(root.clone());
        }
        self.client
            .append_run_event(&self.ctx.kernelfs_run_id, body)
            .await?;
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_context_exposes_kernelfs_run_id() {
        let ctx = KernelfsLifecycleContext {
            kernelfs_run_id: "run_test".into(),
            thread_id: "thread-1".into(),
            workspace_id: Some("ws".into()),
            workspace_root: Some("/tmp/ws".into()),
            base_snapshot_id: "agentd".into(),
            backend: "wasi",
        };
        let emitter = KernelfsLifecycleEmitter::new(
            LatticeToolClient::new("http://127.0.0.1:1", "tok").expect("client"),
            ctx,
        );
        assert_eq!(emitter.context().kernelfs_run_id, "run_test");
    }

    #[test]
    fn collector_drains_in_order() {
        let collector = Arc::new(KernelfsLifecycleCollector::default());
        let hook = collector.hook();
        hook(KernelfsLifecycleStage::Executing);
        hook(KernelfsLifecycleStage::OutputAvailable {
            exit_code: 0,
            draft_count: 1,
        });
        let stages = collector.drain();
        assert_eq!(stages.len(), 2);
        assert!(matches!(stages[0], KernelfsLifecycleStage::Executing));
        assert!(matches!(
            stages[1],
            KernelfsLifecycleStage::OutputAvailable {
                exit_code: 0,
                draft_count: 1
            }
        ));
        assert!(collector.drain().is_empty());
    }
}
