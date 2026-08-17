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
import { useVirtualizer } from "@tanstack/react-virtual";
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
import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

import {
  archiveAgentThread,
  deleteAgentThread,
  displayTitleForThread,
  renameAgentThread,
  type AgentThreadSummary,
} from "../lib/agentThreads";
import { formatAbsoluteTime, formatRelativeTime } from "../lib/relativeTime";
import { workspaceCatalogDisplayName } from "../lib/workspaceCatalog";
import {
  invalidateAgentThreads,
  useAgentThreadsQuery,
} from "../query/useAgentThreadsQuery";
import { useAgentRunActiveQuery } from "../query/useAgentRunStatusQuery";
import { useWorkspaceCatalogQuery } from "../query/useWorkspaceCatalog";
import { useAgentChatControls } from "./agentChatControls";
import { useAgentSessionStore } from "./agentStore";
import {
  normalizeRenameInput,
  selectionAfterThreadRemoval,
  shouldProceedWithDelete,
} from "./agentThreadHistoryActions";
import { sortThreadsWithPins } from "./agentThreadPins";
import "./agentThreadHistory.css";

export interface AgentThreadHistoryProps {
  workspaceRoot: string;
}

const ESTIMATED_ROW_HEIGHT = 48;
const EMPTY_PINNED: readonly string[] = [];

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
  focused: boolean;
  pinned: boolean;
  isActiveRun: boolean;
  disabled: boolean;
  onSelect: () => void;
  onTogglePin: () => void;
  onRename: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onFocusRow: () => void;
};

function AgentThreadRow({
  thread,
  selected,
  focused,
  pinned,
  isActiveRun,
  disabled,
  onSelect,
  onTogglePin,
  onRename,
  onArchive,
  onDelete,
  onFocusRow,
}: AgentThreadRowProps) {
  const title = displayTitleForThread(thread);
  const activityIso = threadActivityIso(thread);

  return (
    <div
      className="agent-thread-browser-item"
      role="option"
      aria-selected={selected}
      id={`agent-thread-option-${thread.id}`}
    >
      <button
        type="button"
        className={`agent-thread-browser-row${selected ? " agent-thread-browser-row-active" : ""}${
          focused ? " agent-thread-browser-row-focused" : ""
        }`}
        tabIndex={focused ? 0 : -1}
        disabled={disabled}
        onClick={onSelect}
        onFocus={onFocusRow}
      >
        <span className="agent-thread-browser-row-main">
          <span className="agent-thread-browser-row-title">
            {pinned ? (
              <PushPin
                size={11}
                weight="fill"
                className="agent-thread-browser-pin-icon"
                aria-hidden="true"
              />
            ) : null}
            {title}
          </span>
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
              tabIndex={focused ? 0 : -1}
            >
              <DotsThree size={14} weight="bold" />
            </IconButton>
          }
        />
        <MenuPortal>
          <MenuPositioner sideOffset={4} align="end">
            <MenuPopup className="ltui-menu agent-thread-browser-actions-menu">
              <MenuItem
                className="ltui-menu-item"
                disabled={disabled}
                onClick={() => {
                  onRename();
                }}
              >
                <PencilSimple size={14} />
                Rename…
              </MenuItem>
              <MenuItem
                className="ltui-menu-item"
                disabled={disabled}
                onClick={() => {
                  onTogglePin();
                }}
              >
                <PushPin size={14} weight={pinned ? "fill" : "regular"} />
                {pinned ? "Unpin" : "Pin"}
              </MenuItem>
              <MenuItem
                className="ltui-menu-item"
                disabled={disabled}
                onClick={() => {
                  onArchive();
                }}
              >
                <Archive size={14} />
                Archive
              </MenuItem>
              <MenuSeparator />
              <MenuItem
                className="ltui-menu-item"
                disabled={disabled}
                onClick={() => {
                  onDelete();
                }}
              >
                <Trash size={14} />
                Delete
              </MenuItem>
            </MenuPopup>
          </MenuPositioner>
        </MenuPortal>
      </MenuRoot>
    </div>
  );
}

export function AgentThreadHistory({ workspaceRoot }: AgentThreadHistoryProps) {
  const listId = useId();
  const queryClient = useQueryClient();
  const [searchQuery, setSearchQuery] = useState("");
  const [focusIndex, setFocusIndex] = useState(0);
  const [actionError, setActionError] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const threadId = useAgentSessionStore((state) => state.threadIds[workspaceRoot] ?? "");
  const threadListEpoch = useAgentSessionStore((state) => state.threadListEpoch);
  const pinnedIds = useAgentSessionStore(
    (state) => state.pinnedThreadIds[normalizeWorkspaceRoot(workspaceRoot)] ?? EMPTY_PINNED,
  );
  const selectThreadId = useAgentSessionStore((state) => state.selectThreadId);
  const startNewThread = useAgentSessionStore((state) => state.startNewThread);
  const togglePinnedThread = useAgentSessionStore((state) => state.togglePinnedThread);

  const controls = useAgentChatControls();
  const isStreaming = controls?.isStreaming === true;
  const { data: durableActiveRun } = useAgentRunActiveQuery(workspaceRoot, threadId || null);
  const durableRunActive = durableActiveRun?.run?.status === "running";

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
    const options = threads.slice();
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

  const visibleThreads = useMemo(() => {
    const filtered = allThreads.filter((thread) => threadMatchesQuery(thread, searchQuery));
    return sortThreadsWithPins(filtered, pinnedIds);
  }, [allThreads, pinnedIds, searchQuery]);

  useEffect(() => {
    setFocusIndex((current) => {
      if (visibleThreads.length === 0) {
        return 0;
      }
      const selectedIndex = visibleThreads.findIndex((thread) => thread.id === threadId);
      if (selectedIndex >= 0) {
        return selectedIndex;
      }
      return Math.min(current, visibleThreads.length - 1);
    });
  }, [threadId, visibleThreads]);

  const virtualizer = useVirtualizer({
    count: visibleThreads.length,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    getItemKey: (index) => visibleThreads[index]!.id,
    getScrollElement: () => listRef.current,
    overscan: 6,
  });

  const interactionsDisabled = isStreaming;

  const applySelectionAfterRemoval = useCallback(
    (removedThreadId: string) => {
      const remainingThreadIds = visibleThreads.map((thread) => thread.id);
      const nextSelection = selectionAfterThreadRemoval(
        removedThreadId,
        threadId,
        remainingThreadIds,
      );
      switch (nextSelection.kind) {
        case "unchanged":
          return;
        case "select":
          selectThreadId(workspaceRoot, nextSelection.threadId);
          return;
        case "new":
          startNewThread(workspaceRoot);
          return;
        default: {
          const _exhaustive: never = nextSelection;
          return _exhaustive;
        }
      }
    },
    [selectThreadId, startNewThread, threadId, visibleThreads, workspaceRoot],
  );

  const handleRenameThread = useCallback(
    async (thread: AgentThreadSummary) => {
      if (interactionsDisabled) {
        return;
      }
      const currentTitle = displayTitleForThread(thread);
      const nextTitle = normalizeRenameInput(
        window.prompt("Rename thread", currentTitle),
      );
      if (!nextTitle || nextTitle === currentTitle) {
        return;
      }
      setActionError(null);
      try {
        await renameAgentThread({
          workspaceRoot,
          threadId: thread.id,
          title: nextTitle,
        });
        await invalidateAgentThreads(queryClient, workspaceRoot);
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err));
      }
    },
    [interactionsDisabled, queryClient, workspaceRoot],
  );

  const handleArchiveThread = useCallback(
    async (thread: AgentThreadSummary) => {
      if (interactionsDisabled) {
        return;
      }
      setActionError(null);
      try {
        await archiveAgentThread({
          workspaceRoot,
          threadId: thread.id,
        });
        applySelectionAfterRemoval(thread.id);
        await invalidateAgentThreads(queryClient, workspaceRoot);
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err));
      }
    },
    [applySelectionAfterRemoval, interactionsDisabled, queryClient, workspaceRoot],
  );

  const handleDeleteThread = useCallback(
    async (thread: AgentThreadSummary) => {
      if (interactionsDisabled) {
        return;
      }
      if (!shouldProceedWithDelete()) {
        return;
      }
      setActionError(null);
      try {
        await deleteAgentThread({
          workspaceRoot,
          threadId: thread.id,
        });
        applySelectionAfterRemoval(thread.id);
        await invalidateAgentThreads(queryClient, workspaceRoot);
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err));
      }
    },
    [applySelectionAfterRemoval, interactionsDisabled, queryClient, workspaceRoot],
  );

  const moveFocus = useCallback(
    (nextIndex: number) => {
      if (visibleThreads.length === 0) {
        return;
      }
      const clamped = Math.max(0, Math.min(visibleThreads.length - 1, nextIndex));
      setFocusIndex(clamped);
      virtualizer.scrollToIndex(clamped, { align: "auto" });
      requestAnimationFrame(() => {
        const option = listRef.current?.querySelector<HTMLElement>(
          `#agent-thread-option-${CSS.escape(visibleThreads[clamped]!.id)} button`,
        );
        option?.focus();
      });
    },
    [virtualizer, visibleThreads],
  );

  const onListKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (interactionsDisabled || visibleThreads.length === 0) {
        return;
      }
      switch (event.key) {
        case "ArrowDown":
          event.preventDefault();
          moveFocus(focusIndex + 1);
          break;
        case "ArrowUp":
          event.preventDefault();
          moveFocus(focusIndex - 1);
          break;
        case "Home":
          event.preventDefault();
          moveFocus(0);
          break;
        case "End":
          event.preventDefault();
          moveFocus(visibleThreads.length - 1);
          break;
        case "Enter":
        case " ": {
          event.preventDefault();
          const thread = visibleThreads[focusIndex];
          if (thread && thread.id !== threadId) {
            selectThreadId(workspaceRoot, thread.id);
          }
          break;
        }
        default:
          break;
      }
    },
    [
      focusIndex,
      interactionsDisabled,
      moveFocus,
      selectThreadId,
      threadId,
      visibleThreads,
      workspaceRoot,
    ],
  );

  const virtualItems = virtualizer.getVirtualItems();
  const activeDescendant =
    visibleThreads[focusIndex] != null
      ? `agent-thread-option-${visibleThreads[focusIndex]!.id}`
      : undefined;

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

      <div
        ref={listRef}
        id={listId}
        className="agent-thread-browser-list agent-thread-browser-list-virtual"
        role="listbox"
        aria-label="Thread list"
        aria-activedescendant={activeDescendant}
        tabIndex={visibleThreads.length === 0 ? -1 : 0}
        onKeyDown={onListKeyDown}
      >
        {visibleThreads.length === 0 ? (
          <div className="agent-thread-browser-empty" role="status">
            {searchQuery.trim() ? "No matching threads" : "No threads yet"}
          </div>
        ) : (
          <div
            className="agent-thread-browser-virtual-spacer"
            style={{ height: virtualizer.getTotalSize() }}
          >
            {virtualItems.map((item) => {
              const thread = visibleThreads[item.index]!;
              return (
                <div
                  key={item.key}
                  className="agent-thread-browser-virtual-row"
                  data-index={item.index}
                  ref={virtualizer.measureElement}
                  style={{
                    transform: `translateY(${item.start}px)`,
                  }}
                >
                  <AgentThreadRow
                    thread={thread}
                    selected={thread.id === threadId}
                    focused={item.index === focusIndex}
                    pinned={pinnedIds.includes(thread.id)}
                    isActiveRun={
                      thread.id === threadId && (isStreaming || durableRunActive)
                    }
                    disabled={interactionsDisabled}
                    onSelect={() => {
                      if (thread.id !== threadId) {
                        selectThreadId(workspaceRoot, thread.id);
                      }
                    }}
                    onTogglePin={() => {
                      togglePinnedThread(workspaceRoot, thread.id);
                    }}
                    onRename={() => {
                      void handleRenameThread(thread);
                    }}
                    onArchive={() => {
                      void handleArchiveThread(thread);
                    }}
                    onDelete={() => {
                      void handleDeleteThread(thread);
                    }}
                    onFocusRow={() => {
                      setFocusIndex(item.index);
                    }}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>

      {loadError ? (
        <p className="agent-thread-browser-error" role="status">
          History unavailable
        </p>
      ) : null}
      {actionError ? (
        <p className="agent-thread-browser-error" role="status">
          {actionError}
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
