import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ExecutionResult } from "./executionContracts";

/** Declared task I/O binding from `task.yaml` (camelCase IPC). */
export interface TaskIoBinding {
  path: string;
  kind?: string;
}

/** Validated task manifest DTO from `task_load_manifest`. */
export interface TaskManifestDto {
  format: string;
  version: number;
  runtime: {
    type: string;
    provider: string;
    project: string;
  };
  entrypoint: {
    command: string[];
  };
  limits: {
    timeoutSeconds: number;
  };
  inputs: TaskIoBinding[];
  outputs: TaskIoBinding[];
}

export type TaskRunResponse = {
  executionId: string;
};

const TASK_EXECUTION_EVENT = "task-execution-updated";

/** Load and validate `task.yaml` for a workspace-relative `.task/` package. */
export function loadTaskManifest(root: string, relPath: string): Promise<TaskManifestDto> {
  return invoke<TaskManifestDto>("task_load_manifest", {
    request: { root, relPath },
  });
}

/** Start a background task run; returns immediately with an execution id. */
export function runTask(
  root: string,
  relPath: string,
  executionId?: string,
): Promise<TaskRunResponse> {
  return invoke<TaskRunResponse>("task_run", {
    request: {
      root,
      relPath,
      ...(executionId ? { executionId } : {}),
    },
  });
}

/** Process-group kill an in-flight execution. */
export function cancelTask(executionId: string): Promise<void> {
  return invoke<void>("task_cancel", {
    request: { executionId },
  });
}

/** Poll the current execution result. */
export function getTaskExecutionStatus(executionId: string): Promise<ExecutionResult> {
  return invoke<ExecutionResult>("task_execution_status", {
    request: { executionId },
  });
}

/** Subscribe to execution status updates (running → terminal). */
export async function listenTaskExecutionUpdates(
  onUpdate: (result: ExecutionResult) => void,
): Promise<UnlistenFn> {
  return listen<ExecutionResult>(TASK_EXECUTION_EVENT, (event) => {
    onUpdate(event.payload);
  });
}

/**
 * Map a polled/emitted execution into the shared H0 `ExecutionResult` shape.
 * Identity for now — kept as an explicit boundary so UI never treats runs as
 * undo-journal entries.
 */
export function toExecutionResult(result: ExecutionResult): ExecutionResult {
  return result;
}
