import { Button } from "@lattice/ui";
import { Plus } from "@phosphor-icons/react";

import {
  displayTitleForThread,
  type AgentThreadSummary,
} from "../lib/agentThreads";
import { useAgentThreadsQuery } from "../query/useAgentThreadsQuery";
import { useAgentChatControls } from "./agentChatControls";
import { useAgentSessionStore } from "./agentStore";

export interface AgentThreadHistoryProps {
  workspaceRoot: string;
}

export function AgentThreadHistory({ workspaceRoot }: AgentThreadHistoryProps) {
  const threadId = useAgentSessionStore((state) => state.threadIds[workspaceRoot] ?? "");
  const selectThreadId = useAgentSessionStore((state) => state.selectThreadId);
  const startNewThread = useAgentSessionStore((state) => state.startNewThread);
  const controls = useAgentChatControls();
  const isStreaming = controls?.isStreaming === true;

  const { data: threads = [], error } = useAgentThreadsQuery(workspaceRoot);
  const loadError = error instanceof Error ? error.message : error ? String(error) : null;

  const options: AgentThreadSummary[] = threads.slice().sort((a, b) => b.updatedAt - a.updatedAt);
  const knownIds = new Set(options.map((thread) => thread.id));
  if (threadId && !knownIds.has(threadId)) {
    options.unshift({
      id: threadId,
      title: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    });
  }

  return (
    <div className="agent-thread-history">
      <label className="agent-thread-history-field">
        <span className="agent-thread-history-label">Thread</span>
        <select
          className="agent-thread-history-select"
          aria-label="Agent thread"
          value={threadId}
          disabled={isStreaming || options.length === 0}
          onChange={(event) => {
            const next = event.currentTarget.value;
            if (next && next !== threadId) {
              selectThreadId(workspaceRoot, next);
            }
          }}
        >
          {options.length === 0 ? (
            <option value={threadId || ""}>New thread</option>
          ) : (
            options.map((thread) => (
              <option key={thread.id} value={thread.id}>
                {displayTitleForThread(thread)}
              </option>
            ))
          )}
        </select>
      </label>
      <Button
        variant="ghost"
        size="sm"
        className="agent-thread-history-new"
        disabled={isStreaming}
        onClick={() => {
          startNewThread(workspaceRoot);
        }}
      >
        <Plus size={13} />
        New
      </Button>
      {loadError ? (
        <span className="agent-thread-history-error" role="status">
          History unavailable
        </span>
      ) : null}
    </div>
  );
}
