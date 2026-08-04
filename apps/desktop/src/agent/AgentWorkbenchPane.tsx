import type { ReactNode } from "react";

import type { TransactionProposalSummary } from "../lib/proposals";
import { proposalStatusLabel } from "../lib/proposals";
import { useAgentSessionStore, type TrailStep } from "./agentStore";
import { AgentTrail } from "./AgentTrail";
import {
  approvalTrailSteps,
  changeTrailSteps,
  formatWorkbenchTrailKind,
  pendingApprovalProposals,
  planTrailSteps,
  trailStepKey,
  workbenchTrailDetail,
} from "./agentWorkbenchSections";
import {
  formatAgentRunStatusLabel,
  useAgentRunWorkbenchModel,
} from "./useAgentRunWorkbenchModel";

function WorkbenchSection({
  title,
  emptyMessage,
  children,
  headerAside,
}: {
  title: string;
  emptyMessage: string;
  children?: ReactNode;
  headerAside?: ReactNode;
}) {
  const hasContent = Boolean(children);
  return (
    <section className="agent-workbench-section">
      <header className="agent-workbench-section-head">
        <h3>{title}</h3>
        {headerAside}
      </header>
      {hasContent ? (
        <div className="agent-workbench-section-body">{children}</div>
      ) : (
        <p className="agent-workbench-empty">{emptyMessage}</p>
      )}
    </section>
  );
}

function WorkbenchTrailRows({ steps }: { steps: readonly TrailStep[] }) {
  return (
    <ul className="agent-workbench-row-list">
      {steps.map((step) => (
        <li key={trailStepKey(step)} className="agent-workbench-row">
          <span className="agent-workbench-row-kind">{formatWorkbenchTrailKind(step.kind)}</span>
          <span className="agent-workbench-row-label">{workbenchTrailDetail(step)}</span>
          <span
            className={`agent-workbench-row-status agent-workbench-row-status-${step.status.replace("_", "-")}`}
            aria-label={step.status === "in_progress" ? "In progress" : "Completed"}
          />
        </li>
      ))}
    </ul>
  );
}

export interface AgentWorkbenchPaneProps {
  workspaceRoot?: string | null;
  threadId?: string | null;
  proposals?: readonly TransactionProposalSummary[];
  proposalLoading?: boolean;
  onOpenProposal?: (proposalId: string) => void | Promise<void>;
  /** Highlight the proposal currently open in the Current|Proposed split. */
  activeProposalId?: string | null;
}

export function AgentWorkbenchPane({
  workspaceRoot = null,
  threadId = null,
  proposals = [],
  proposalLoading = false,
  onOpenProposal,
  activeProposalId = null,
}: AgentWorkbenchPaneProps) {
  const storeThreadId = useAgentSessionStore((state) =>
    workspaceRoot ? (state.threadIds[workspaceRoot] ?? null) : null,
  );
  const resolvedThreadId = threadId ?? storeThreadId;
  const { status, projection } = useAgentRunWorkbenchModel(workspaceRoot, resolvedThreadId);

  const trailSteps = useAgentSessionStore((state) => state.trailSteps);
  const evidence = useAgentSessionStore((state) => state.evidence);

  const planSteps = planTrailSteps(trailSteps);
  const changeSteps = changeTrailSteps(trailSteps);
  const approvalSteps = approvalTrailSteps(trailSteps);
  const pendingProposals = pendingApprovalProposals(proposals);
  const lifecycle = projection?.lifecycle ?? [];
  const statusLabel = formatAgentRunStatusLabel(status);
  const statusChip = statusLabel ? (
    <span
      className={`agent-workbench-run-chip agent-workbench-run-chip-${status}`}
      role="status"
      aria-label={`Run status: ${statusLabel}`}
    >
      {statusLabel}
    </span>
  ) : null;

  return (
    <aside className="agent-workbench-pane" aria-label="Agent workbench">
      <WorkbenchSection
        title="Timeline"
        emptyMessage="Trail steps appear as the agent works."
        headerAside={statusChip}
      >
        {trailSteps.length > 0 ? <AgentTrail /> : null}
      </WorkbenchSection>

      <WorkbenchSection
        title="Execution"
        emptyMessage="KernelFS run lifecycle events will appear here."
      >
        {lifecycle.length > 0 ? (
          <ol className="agent-workbench-row-list agent-workbench-lifecycle-list">
            {lifecycle.map((row) => (
              <li key={row.eventId} className="agent-workbench-row">
                <span className="agent-workbench-row-kind">
                  {row.eventType.replace(/^run\./, "")}
                </span>
                <span className="agent-workbench-row-label">{row.label}</span>
              </li>
            ))}
          </ol>
        ) : null}
      </WorkbenchSection>

      <WorkbenchSection
        title="Evidence"
        emptyMessage="Retrieved workspace evidence will appear here."
      >
        {evidence.length > 0 ? (
          <ul className="agent-workbench-evidence-list">
            {evidence.map((item) => (
              <li key={`${item.runId}:${item.evidenceId}`} className="agent-workbench-evidence-item">
                <code className="agent-workbench-evidence-path">{item.path}</code>
                <p className="agent-workbench-evidence-excerpt">{item.excerpt}</p>
              </li>
            ))}
          </ul>
        ) : null}
      </WorkbenchSection>

      <WorkbenchSection
        title="Plan"
        emptyMessage="Model planning steps appear here during a run."
      >
        {planSteps.length > 0 ? <WorkbenchTrailRows steps={planSteps} /> : null}
      </WorkbenchSection>

      <WorkbenchSection
        title="Changes"
        emptyMessage="Proposed workspace edits appear here during a run."
      >
        {changeSteps.length > 0 ? <WorkbenchTrailRows steps={changeSteps} /> : null}
      </WorkbenchSection>

      <WorkbenchSection
        title="Approvals"
        emptyMessage="Pending proposals and approval pauses appear here."
      >
        {proposalLoading ? (
          <p className="agent-workbench-empty" role="status">Loading approvals…</p>
        ) : approvalSteps.length > 0 || pendingProposals.length > 0 ? (
          <>
            {approvalSteps.length > 0 ? <WorkbenchTrailRows steps={approvalSteps} /> : null}
            {pendingProposals.length > 0 ? (
              <ul className="agent-workbench-row-list">
                {pendingProposals.map((item) => {
                  const pathHint =
                    item.affectedPaths.length > 0 ? item.affectedPaths.slice(0, 2).join(", ") : null;
                  const canOpen = Boolean(onOpenProposal);
                  return (
                    <li key={item.id}>
                      {canOpen ? (
                        <button
                          type="button"
                          className={`agent-workbench-approval-item${
                            activeProposalId === item.id ? " is-active" : ""
                          }`}
                          aria-pressed={activeProposalId === item.id}
                          onClick={() => void onOpenProposal?.(item.id)}
                        >
                          <span className="agent-workbench-approval-head">
                            <strong>{item.summary}</strong>
                            <span className="agent-workbench-approval-status">
                              {proposalStatusLabel(item.status)}
                            </span>
                          </span>
                          <small>
                            {item.commandCount} command{item.commandCount === 1 ? "" : "s"} ·{" "}
                            {item.source.type}
                            {pathHint ? ` · ${pathHint}` : ""}
                          </small>
                        </button>
                      ) : (
                        <div className="agent-workbench-row agent-workbench-approval-static">
                          <span className="agent-workbench-row-kind">proposal</span>
                          <span className="agent-workbench-row-label">{item.summary}</span>
                          <span className="agent-workbench-row-meta">
                            {proposalStatusLabel(item.status)}
                          </span>
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
            ) : null}
          </>
        ) : null}
      </WorkbenchSection>
    </aside>
  );
}
