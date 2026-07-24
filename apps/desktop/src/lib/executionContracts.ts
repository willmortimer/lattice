/**
 * Shared IPC contracts for command-side effects beyond the undo journal.
 *
 * - **Commands** (semantic transactions): mutations recorded in history for
 *   undo/redo and audit — see Rust `lattice_commands::Command`.
 * - **Executions** (`ExecutionResult`): long-running jobs with stdout/stderr
 *   and materialized outputs.
 * - **Proposals** (`TransactionProposal`): reviewable command bundles produced
 *   by tasks, MCP, or external agents before application.
 * - **Bindings** (`BindingSpec`): how interface/embed components load data.
 */

export type {
  BindingSpec,
  InterfaceComponent,
  InterfaceComponentType,
  InterfaceDef,
  InterfaceLayout,
  InterfaceParameter,
} from "./bindingSpec";
export {
  interfaceHasDashboardComponents,
  isBindingSpec,
} from "./bindingSpec";

export interface ResourceOutput {
  path: string;
  kind?: string;
  hash?: string;
}

export type ExecutionStatus =
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  /** Owning process exited before the run finished (daemon restart). */
  | "abandoned";

export interface ExecutionResult {
  id: string;
  status: ExecutionStatus;
  stdout: string;
  stderr: string;
  /** ISO-8601 */
  startedAt: string;
  /** ISO-8601 */
  finishedAt?: string;
  outputs: ResourceOutput[];
  /** First proposal id (compat); prefer `proposalIds` when multiple. */
  proposalId?: string;
  /** All proposal ids produced by this execution (parallel-safe). */
  proposalIds?: string[];
}

export type ProposalSourceType =
  | "task"
  | "workflow"
  | "artifact"
  | "mcp"
  | "external";

export interface ProposalSource {
  type: ProposalSourceType;
  resource?: string;
  executionId?: string;
  stepId?: string;
}

export type ProposalStatus = "pending" | "accepted" | "rejected";

export interface TransactionProposal {
  id: string;
  source: ProposalSource;
  summary: string;
  /** Serialized `Command` JSON with kebab-case `type` tags — see Rust. */
  commands: unknown[];
  affectedPaths: string[];
  warnings: string[];
  /** ISO-8601 */
  createdAt: string;
  /** Defaults to pending when omitted (older payloads). */
  status?: ProposalStatus;
  /** ISO-8601 when accepted or rejected. */
  resolvedAt?: string;
  /** History transaction id when accepted. */
  appliedTransactionId?: string;
}

export interface TransactionProposalSummary {
  id: string;
  source: ProposalSource;
  summary: string;
  commandCount: number;
  affectedPaths: string[];
  warnings: string[];
  /** ISO-8601 */
  createdAt: string;
  status: ProposalStatus;
  resolvedAt?: string;
  appliedTransactionId?: string;
}

/** Bounded per-command detail for proposal review. */
export type CommandPreviewDetail =
  | {
      kind: "text-create";
      path: string;
      contentExcerpt: string;
      truncated: boolean;
      byteLen: number;
    }
  | {
      kind: "text-diff";
      path: string;
      beforeExcerpt?: string;
      afterExcerpt: string;
      truncated: boolean;
    }
  | {
      kind: "record-change";
      path: string;
      table: string;
      operation: string;
      id?: string;
      fieldSummary: string;
    }
  | {
      kind: "workflow-summary";
      path: string;
      name?: string;
      stepCount?: number;
      excerpt: string;
      truncated: boolean;
    }
  | {
      kind: "interface-summary";
      path: string;
      name?: string;
      title?: string;
      componentCount?: number;
      excerpt: string;
      truncated: boolean;
    }
  | {
      kind: "artifact-summary";
      path: string;
      title?: string;
      entrypoint?: string;
      excerpt: string;
      truncated: boolean;
    }
  | {
      kind: "file-op";
      operation: string;
      paths: string[];
      metadata?: Record<string, string>;
    };

export interface CommandPreview {
  index: number;
  commandType: string;
  summary: string;
  touchedPaths: string[];
  warnings: string[];
  detail?: CommandPreviewDetail;
}

export interface ProposalPreview {
  proposalId: string;
  commands: CommandPreview[];
  subsetValid: boolean;
  subsetErrors: string[];
  missingPredecessors: number[];
}
