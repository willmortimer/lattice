import {
  ComposerPrimitive,
  ErrorPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  useAuiState,
} from "@assistant-ui/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Button } from "@lattice/ui";
import {
  type ComponentProps,
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
} from "react";

import { SemanticToolCall } from "./tools/semanticToolRegistry";
import { AgentContextChips } from "./AgentContextChips";

const ESTIMATED_TURN_HEIGHT = 160;
const AT_BOTTOM_THRESHOLD = 4;

type MessageComponents = ComponentProps<
  typeof ThreadPrimitive.Unstable_MessageById
>["components"];

type MessageRow = {
  id: string;
  role: "user" | "assistant" | "system";
};

type Turn = { id: string; messageIds: string[] };

function useThreadMessageRows(): readonly MessageRow[] {
  const prevRowsRef = useRef<readonly MessageRow[]>([]);

  return useAuiState((state) => {
    const messages = state.thread.messages;
    const prev = prevRowsRef.current;
    if (
      prev.length === messages.length &&
      prev.every((row, index) => {
        const message = messages[index]!;
        return row.id === message.id && row.role === message.role;
      })
    ) {
      return prev;
    }

    const next = messages.map(({ id, role }) => ({ id, role }));
    prevRowsRef.current = next;
    return next;
  });
}

function buildTurns(messages: readonly MessageRow[]): Turn[] {
  if (messages.length === 0) {
    return [];
  }

  const turns: Turn[] = [];
  for (const { id, role } of messages) {
    const last = turns.at(-1);
    if (role === "user" || !last) {
      turns.push({ id, messageIds: [id] });
    } else {
      last.messageIds.push(id);
    }
  }
  return turns;
}

function AgentUserMessage() {
  return (
    <MessagePrimitive.Root className="agent-message agent-message-user">
      <MessagePrimitive.Parts />
    </MessagePrimitive.Root>
  );
}

function AgentAssistantMessage() {
  return (
    <MessagePrimitive.Root className="agent-message agent-message-assistant">
      <MessagePrimitive.Parts
        components={{
          tools: {
            Override: SemanticToolCall,
          },
        }}
      />
      <MessagePrimitive.Error>
        <ErrorPrimitive.Root className="agent-message-error">
          <ErrorPrimitive.Message />
        </ErrorPrimitive.Root>
      </MessagePrimitive.Error>
    </MessagePrimitive.Root>
  );
}

const MESSAGE_COMPONENTS: MessageComponents = {
  UserMessage: AgentUserMessage,
  AssistantMessage: AgentAssistantMessage,
};

function AgentThreadComposer({
  workspaceRoot,
  activeResourcePath,
  onNotify,
}: {
  workspaceRoot: string | null;
  activeResourcePath: string | null;
  onNotify?: (message: string) => void;
}) {
  return (
    <ComposerPrimitive.Root className="agent-composer">
      <AgentContextChips
        workspaceRoot={workspaceRoot}
        activeResourcePath={activeResourcePath}
        onStubAction={onNotify}
      />
      <ComposerPrimitive.Input
        className="agent-composer-input"
        placeholder="Message the agent…"
        rows={3}
      />
      <ComposerPrimitive.Send asChild>
        <Button variant="primary" size="sm" className="agent-composer-send">
          Send
        </Button>
      </ComposerPrimitive.Send>
    </ComposerPrimitive.Root>
  );
}

export type VirtualizedAgentThreadViewProps = {
  paidCta: { title: string; body: string } | null;
  emptyMessage: string;
  composerDisabled: boolean;
  workspaceRoot: string | null;
  activeResourcePath: string | null;
  onNotify?: (message: string) => void;
  header?: ReactNode;
};

export function VirtualizedAgentThreadView({
  paidCta,
  emptyMessage,
  composerDisabled,
  workspaceRoot,
  activeResourcePath,
  onNotify,
  header,
}: VirtualizedAgentThreadViewProps) {
  const messageRows = useThreadMessageRows();
  const isRunning = useAuiState((state) => state.thread.isRunning);
  const turns = useMemo(() => buildTurns(messageRows), [messageRows]);

  const scrollerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const stickyRef = useRef(true);

  const virtualizer = useVirtualizer({
    count: turns.length,
    estimateSize: () => ESTIMATED_TURN_HEIGHT,
    getItemKey: (index) => turns[index]!.id,
    getScrollElement: () => scrollerRef.current,
    overscan: 4,
    scrollToFn: (offset, _options, instance) => {
      const element = instance.scrollElement;
      if (!element) {
        return;
      }
      if (stickyRef.current) {
        const maxScroll = element.scrollHeight - element.clientHeight;
        if (maxScroll - element.scrollTop <= AT_BOTTOM_THRESHOLD && offset < maxScroll) {
          return;
        }
      }
      element.scrollTo(0, offset);
    },
  });

  const jumpToBottom = useCallback(() => {
    stickyRef.current = true;
    if (turns.length > 0) {
      virtualizer.scrollToIndex(turns.length - 1, { align: "end" });
    }
    requestAnimationFrame(() => {
      const element = scrollerRef.current;
      if (element && stickyRef.current) {
        element.scrollTop = element.scrollHeight;
      }
    });
  }, [turns.length, virtualizer]);

  useEffect(() => {
    const element = scrollerRef.current;
    if (!element) {
      return;
    }

    let lastScrollTop = element.scrollTop;
    let lastScrollHeight = element.scrollHeight;
    let lastClientHeight = element.clientHeight;

    const onScroll = () => {
      const atBottom =
        element.scrollHeight - element.scrollTop - element.clientHeight <= AT_BOTTOM_THRESHOLD;
      if (atBottom) {
        stickyRef.current = true;
      } else if (
        element.scrollTop < lastScrollTop &&
        element.scrollHeight === lastScrollHeight &&
        Math.abs(element.clientHeight - lastClientHeight) <= 1
      ) {
        stickyRef.current = false;
      }
      lastScrollTop = element.scrollTop;
      lastScrollHeight = element.scrollHeight;
      lastClientHeight = element.clientHeight;
    };

    const onWheel = (event: WheelEvent) => {
      if (event.deltaY < 0) {
        stickyRef.current = false;
      }
    };

    const disarm = () => {
      stickyRef.current = false;
    };

    element.addEventListener("scroll", onScroll, { passive: true });
    element.addEventListener("wheel", onWheel, { passive: true });
    element.addEventListener("touchmove", disarm, { passive: true });
    return () => {
      element.removeEventListener("scroll", onScroll);
      element.removeEventListener("wheel", onWheel);
      element.removeEventListener("touchmove", disarm);
    };
  }, []);

  useEffect(() => {
    const element = scrollerRef.current;
    const content = contentRef.current;
    if (!element || !content) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (stickyRef.current) {
        element.scrollTop = element.scrollHeight;
      }
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, []);

  const prevIsRunningRef = useRef(false);
  useLayoutEffect(() => {
    if (isRunning && !prevIsRunningRef.current) {
      jumpToBottom();
    }
    prevIsRunningRef.current = isRunning;
  }, [isRunning, jumpToBottom]);

  const didInitialJumpRef = useRef(false);
  useLayoutEffect(() => {
    if (didInitialJumpRef.current || turns.length === 0) {
      return;
    }
    didInitialJumpRef.current = true;
    jumpToBottom();
  }, [turns.length, jumpToBottom]);

  const items = virtualizer.getVirtualItems();
  const paddingTop = items[0]?.start ?? 0;
  const paddingBottom = Math.max(
    0,
    virtualizer.getTotalSize() - (items.at(-1)?.end ?? 0),
  );

  return (
    <ThreadPrimitive.Root className="agent-thread">
      {header}
      {paidCta ? (
        <div className="agent-paid-cta" role="status">
          <strong>{paidCta.title}</strong>
          <p>{paidCta.body}</p>
        </div>
      ) : null}
      <div ref={scrollerRef} className="agent-thread-viewport">
        <ThreadPrimitive.Empty>
          <div className="agent-thread-empty">
            <p>{emptyMessage}</p>
          </div>
        </ThreadPrimitive.Empty>
        <div
          ref={contentRef}
          className="agent-thread-virtual-content"
          style={{ paddingTop, paddingBottom }}
        >
          {items.map((item) => (
            <div
              key={item.key}
              data-index={item.index}
              ref={virtualizer.measureElement}
              className="agent-thread-turn"
            >
              {turns[item.index]!.messageIds.map((messageId) => (
                <ThreadPrimitive.Unstable_MessageById
                  key={messageId}
                  messageId={messageId}
                  components={MESSAGE_COMPONENTS}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
      {!composerDisabled ? (
        <div className="agent-thread-footer">
          <AgentThreadComposer
            workspaceRoot={workspaceRoot}
            activeResourcePath={activeResourcePath}
            onNotify={onNotify}
          />
        </div>
      ) : null}
    </ThreadPrimitive.Root>
  );
}
