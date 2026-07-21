/**
 * Shared IPC contracts for command-side effects beyond the undo journal.
 *
 * - **Commands** (semantic transactions): mutations recorded in history for
 *   undo/redo and audit — see Rust `lattice_commands::Command`.
 * - **Executions** (`ExecutionResult`): long-running jobs with stdout/stderr
 *   and materialized outputs.
 * - **Proposals** (`TransactionProposal`): reviewable command bundles produced
 *   by tasks, MCP, or external agents before application.
 */

export interface ResourceOutput {
  path: string;
  kind?: string;
  hash?: string;
}

export type ExecutionStatus = "running" | "succeeded" | "failed" | "cancelled";

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
  proposalId?: string;
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
}
