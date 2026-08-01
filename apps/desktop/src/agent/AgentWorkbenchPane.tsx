import type { ReactNode } from "react";

import { useAgentSessionStore } from "./agentStore";
import { AgentTrail } from "./AgentTrail";

function WorkbenchSection({
  title,
  emptyMessage,
  children,
}: {
  title: string;
  emptyMessage: string;
  children?: ReactNode;
}) {
  const hasContent = Boolean(children);
  return (
    <section className="agent-workbench-section">
      <header className="agent-workbench-section-head">
        <h3>{title}</h3>
      </header>
      {hasContent ? (
        <div className="agent-workbench-section-body">{children}</div>
      ) : (
        <p className="agent-workbench-empty">{emptyMessage}</p>
      )}
    </section>
  );
}

export function AgentWorkbenchPane() {
  const trailSteps = useAgentSessionStore((state) => state.trailSteps);
  const evidence = useAgentSessionStore((state) => state.evidence);

  return (
    <aside className="agent-workbench-pane" aria-label="Agent workbench">
      <WorkbenchSection
        title="Timeline"
        emptyMessage="Trail steps appear as the agent works."
      >
        {trailSteps.length > 0 ? <AgentTrail /> : null}
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

      {/* Plan / Changes / Outputs / Approvals / Errors land here when run data exists;
          keep one quiet placeholder instead of five empty section stacks. */}
      <WorkbenchSection
        title="Run details"
        emptyMessage="Plan, proposed changes, outputs, approvals, and errors appear here during a run."
      />
    </aside>
  );
}
