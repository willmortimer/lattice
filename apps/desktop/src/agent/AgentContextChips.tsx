import { resourcePathChipLabel, workspaceChipLabel } from "./agentContextChipLabels";

export interface AgentContextChipsProps {
  activeResourcePath: string | null;
  workspaceRoot: string | null;
  onStubAction?: (message: string) => void;
}

export function AgentContextChips({
  activeResourcePath,
  workspaceRoot,
  onStubAction,
}: AgentContextChipsProps) {
  const pageLabel = resourcePathChipLabel(activeResourcePath);
  const workspaceLabel = workspaceChipLabel(workspaceRoot);

  const notifyStub = (message: string) => {
    onStubAction?.(message);
  };

  return (
    <div className="agent-context-chips" role="toolbar" aria-label="Composer context">
      <button
        type="button"
        className={`agent-context-chip${pageLabel ? " agent-context-chip-active" : ""}`}
        disabled={!pageLabel}
        title={activeResourcePath ?? "No page open"}
      >
        {pageLabel ? `Page: ${pageLabel}` : "Current page"}
      </button>
      <button
        type="button"
        className="agent-context-chip agent-context-chip-stub"
        onClick={() => notifyStub("Selection context is not available yet.")}
        title="Attach editor selection (coming soon)"
      >
        Selection
      </button>
      <button
        type="button"
        className={`agent-context-chip${workspaceLabel ? " agent-context-chip-active" : ""}`}
        disabled={!workspaceLabel}
        title={workspaceRoot ?? "No workspace"}
      >
        {workspaceLabel ? `Workspace: ${workspaceLabel}` : "Workspace"}
      </button>
      <button
        type="button"
        className="agent-context-chip agent-context-chip-add"
        onClick={() => notifyStub("Context picker is not available yet.")}
        title="Add context (coming soon)"
      >
        Add context
      </button>
    </div>
  );
}
