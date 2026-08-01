import {
  Button,
  IconButton,
  MenuItem,
  MenuPopup,
  MenuPortal,
  MenuPositioner,
  MenuRoot,
  MenuSeparator,
  MenuTrigger,
} from "@lattice/ui";
import {
  Archive,
  DotsThree,
  MagnifyingGlass,
  PencilSimple,
  PushPin,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useMemo, useState } from "react";

import {
  displayTitleForThread,
  type AgentThreadSummary,
} from "../lib/agentThreads";
import { formatAbsoluteTime, formatRelativeTime } from "../lib/relativeTime";
import { workspaceCatalogDisplayName } from "../lib/workspaceCatalog";
import {
  invalidateAgentThreads,
  useAgentThreadsQuery,
} from "../query/useAgentThreadsQuery";
import { useWorkspaceCatalogQuery } from "../query/useWorkspaceCatalog";
import { useAgentChatControls } from "./agentChatControls";
import { useAgentSessionStore } from "./agentStore";

export interface AgentThreadHistoryProps {
  workspaceRoot: string;
}

function normalizeWorkspaceRoot(root: string): string {
  return root.replace(/\\/g, "/").replace(/\/+$/, "");
}

function threadMatchesQuery(thread: AgentThreadSummary, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }
  const title = displayTitleForThread(thread).toLowerCase();
  if (title.includes(needle)) {
    return true;
  }
  return thread.id.toLowerCase().includes(needle);
}

function threadActivityIso(thread: AgentThreadSummary): string {
  return new Date(thread.updatedAt).toISOString();
}

type AgentThreadRowProps = {
  thread: AgentThreadSummary;
  selected: boolean;
  isActiveRun: boolean;
  disabled: boolean;
  onSelect: () => void;
};

function AgentThreadRow({
  thread,
  selected,
  isActiveRun,
  disabled,
  onSelect,
}: AgentThreadRowProps) {
  const title = displayTitleForThread(thread);
  const activityIso = threadActivityIso(thread);

  return (
    <li className="agent-thread-browser-item">
      <button
        type="button"
        className={`agent-thread-browser-row${selected ? " agent-thread-browser-row-active" : ""}`}
        aria-current={selected ? "true" : undefined}
        disabled={disabled}
        onClick={onSelect}
      >
        <span className="agent-thread-browser-row-main">
          <span className="agent-thread-browser-row-title">{title}</span>
          <span className="agent-thread-browser-row-meta">
            <time dateTime={activityIso} title={formatAbsoluteTime(activityIso)}>
              {formatRelativeTime(activityIso)}
            </time>
            {isActiveRun ? (
              <span className="agent-thread-browser-run-badge" aria-label="Active run">
                Running
              </span>
            ) : null}
          </span>
        </span>
      </button>
      <MenuRoot>
        <MenuTrigger
          render={
            <IconButton
              label={`Thread actions for ${title}`}
              className="agent-thread-browser-actions-trigger"
              disabled={disabled}
            >
              <DotsThree size={14} weight="bold" />
            </IconButton>
          }
        />
        <MenuPortal>
          <MenuPositioner sideOffset={4} align="end">
            <MenuPopup className="ltui-menu agent-thread-browser-actions-menu">
              <MenuItem className="ltui-menu-item" disabled>
                <PencilSimple size={14} />
                Rename…
              </MenuItem>
              <MenuItem className="ltui-menu-item" disabled>
                <PushPin size={14} />
                Pin
              </MenuItem>
              <MenuItem className="ltui-menu-item" disabled>
                <Archive size={14} />
                Archive
              </MenuItem>
              <MenuSeparator />
              <MenuItem className="ltui-menu-item" disabled>
                <Trash size={14} />
                Delete
              </MenuItem>
            </MenuPopup>
          </MenuPositioner>
        </MenuPortal>
      </MenuRoot>
    </li>
  );
}

export function AgentThreadHistory({ workspaceRoot }: AgentThreadHistoryProps) {
  const listId = useId();
  const queryClient = useQueryClient();
  const [searchQuery, setSearchQuery] = useState("");

  const threadId = useAgentSessionStore((state) => state.threadIds[workspaceRoot] ?? "");
  const threadListEpoch = useAgentSessionStore((state) => state.threadListEpoch);
  const selectThreadId = useAgentSessionStore((state) => state.selectThreadId);
  const startNewThread = useAgentSessionStore((state) => state.startNewThread);

  const controls = useAgentChatControls();
  const isStreaming = controls?.isStreaming === true;

  const { data: threads = [], error, isFetching } = useAgentThreadsQuery(workspaceRoot);
  const { data: workspaceCatalog } = useWorkspaceCatalogQuery();

  useEffect(() => {
    if (threadListEpoch > 0) {
      void invalidateAgentThreads(queryClient, workspaceRoot);
    }
  }, [queryClient, threadListEpoch, workspaceRoot]);

  const loadError = error instanceof Error ? error.message : error ? String(error) : null;

  const workspaceLabel = useMemo(() => {
    const normalizedRoot = normalizeWorkspaceRoot(workspaceRoot);
    const entry = workspaceCatalog?.workspaces.find(
      (workspace) => normalizeWorkspaceRoot(workspace.root) === normalizedRoot,
    );
    if (entry) {
      return workspaceCatalogDisplayName(entry);
    }
    const parts = normalizedRoot.split("/").filter(Boolean);
    return parts[parts.length - 1] ?? "Workspace";
  }, [workspaceCatalog, workspaceRoot]);

  const allThreads = useMemo(() => {
    const options = threads.slice().sort((a, b) => b.updatedAt - a.updatedAt);
    const knownIds = new Set(options.map((thread) => thread.id));
    if (threadId && !knownIds.has(threadId)) {
      options.unshift({
        id: threadId,
        title: null,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      });
    }
    return options;
  }, [threadId, threads]);

  const visibleThreads = useMemo(
    () => allThreads.filter((thread) => threadMatchesQuery(thread, searchQuery)),
    [allThreads, searchQuery],
  );

  const interactionsDisabled = isStreaming;

  return (
    <section className="agent-thread-browser" aria-label="Agent threads">
      <div className="agent-thread-browser-context">
        <span className="agent-thread-browser-eyebrow">Workspace</span>
        <span className="agent-thread-browser-workspace" title={workspaceRoot}>
          {workspaceLabel}
        </span>
      </div>

      <div className="agent-thread-browser-toolbar">
        <label className="agent-thread-browser-search">
          <MagnifyingGlass size={13} aria-hidden="true" className="agent-thread-browser-search-icon" />
          <input
            type="search"
            className="agent-thread-browser-search-input"
            placeholder="Search threads"
            aria-controls={listId}
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.currentTarget.value)}
          />
        </label>
        <Button
          variant="ghost"
          size="sm"
          className="agent-thread-browser-new"
          disabled={interactionsDisabled}
          onClick={() => {
            startNewThread(workspaceRoot);
            setSearchQuery("");
          }}
        >
          <Plus size={13} />
          New
        </Button>
      </div>

      <ul id={listId} className="agent-thread-browser-list" role="list">
        {visibleThreads.length === 0 ? (
          <li className="agent-thread-browser-empty" role="status">
            {searchQuery.trim() ? "No matching threads" : "No threads yet"}
          </li>
        ) : (
          visibleThreads.map((thread) => (
            <AgentThreadRow
              key={thread.id}
              thread={thread}
              selected={thread.id === threadId}
              isActiveRun={isStreaming && thread.id === threadId}
              disabled={interactionsDisabled}
              onSelect={() => {
                if (thread.id !== threadId) {
                  selectThreadId(workspaceRoot, thread.id);
                }
              }}
            />
          ))
        )}
      </ul>

      {loadError ? (
        <p className="agent-thread-browser-error" role="status">
          History unavailable
        </p>
      ) : null}
      {isFetching && !loadError ? (
        <p className="agent-thread-browser-status" role="status" aria-live="polite">
          Refreshing threads…
        </p>
      ) : null}
    </section>
  );
}
