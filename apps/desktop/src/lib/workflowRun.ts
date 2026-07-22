import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ExecutionResult, ExecutionStatus } from "./executionContracts";

/** Declared workflow step from `*.workflow.yaml` (camelCase IPC). */
export interface WorkflowStepDto {
  id: string;
  action: string;
  with: unknown;
}

/** Trigger summary for the workflow surface. */
export interface WorkflowTriggerDto {
  type: string;
  paths?: string[];
  form?: string;
  package?: string;
  formId?: string;
  intervalSeconds?: number;
  cron?: string;
  timezone?: string;
}

/** Validated workflow manifest DTO from `workflow_load`. */
export interface WorkflowManifestDto {
  format: string;
  version: number;
  name: string;
  enabled: boolean;
  trigger: WorkflowTriggerDto;
  steps: WorkflowStepDto[];
  rawYaml: string;
}

export interface WorkflowStepResultDto {
  id: string;
  action: string;
  status: ExecutionStatus;
  log: string;
  proposalId?: string;
}

/** Full run record emitted by `workflow-execution-updated`. */
export interface WorkflowRunRecordDto {
  workflowPath: string;
  trigger: string;
  execution: ExecutionResult;
  steps: WorkflowStepResultDto[];
}

export type WorkflowRunResponse = {
  executionId: string;
};

const WORKFLOW_EXECUTION_EVENT = "workflow-execution-updated";

/** Load and validate a workspace-relative `*.workflow.yaml`. */
export function loadWorkflow(root: string, relPath: string): Promise<WorkflowManifestDto> {
  return invoke<WorkflowManifestDto>("workflow_load", {
    request: { root, relPath },
  });
}

/** Start a background workflow run; returns immediately with an execution id. */
export function runWorkflow(
  root: string,
  relPath: string,
  options?: { executionId?: string; trigger?: string },
): Promise<WorkflowRunResponse> {
  return invoke<WorkflowRunResponse>("workflow_run", {
    request: {
      root,
      relPath,
      ...(options?.executionId ? { executionId: options.executionId } : {}),
      ...(options?.trigger ? { trigger: options.trigger } : {}),
    },
  });
}

/** Request cancellation between steps (best-effort). */
export function cancelWorkflow(executionId: string): Promise<void> {
  return invoke<void>("workflow_cancel", {
    request: { executionId },
  });
}

/** Poll the current run record. */
export function getWorkflowExecutionStatus(executionId: string): Promise<WorkflowRunRecordDto> {
  return invoke<WorkflowRunRecordDto>("workflow_execution_status", {
    request: { executionId },
  });
}

/** Persist `enabled` on the workflow YAML. */
export function setWorkflowEnabled(
  root: string,
  relPath: string,
  enabled: boolean,
): Promise<WorkflowManifestDto> {
  return invoke<WorkflowManifestDto>("workflow_set_enabled", {
    request: { root, relPath, enabled },
  });
}

/** Recent run history for a workflow path. */
export function listWorkflowRuns(
  root: string,
  relPath: string,
  limit = 20,
): Promise<WorkflowRunRecordDto[]> {
  return invoke<WorkflowRunRecordDto[]>("workflow_list_runs", {
    request: { root, relPath, limit },
  });
}

/** Subscribe to workflow execution updates. */
export async function listenWorkflowExecutionUpdates(
  onUpdate: (record: WorkflowRunRecordDto) => void,
): Promise<UnlistenFn> {
  return listen<WorkflowRunRecordDto>(WORKFLOW_EXECUTION_EVENT, (event) => {
    onUpdate(event.payload);
  });
}

/**
 * Map a polled/emitted run into the shared H0 `ExecutionResult` shape.
 * Identity for the nested execution — kept as an explicit boundary.
 */
export function toExecutionResult(record: WorkflowRunRecordDto): ExecutionResult {
  return record.execution;
}
