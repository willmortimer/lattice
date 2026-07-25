import { useChat } from "@ai-sdk/react";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { useAISDKRuntime } from "@assistant-ui/react-ai-sdk";
import { useEffect, useMemo, type ReactNode } from "react";

import { getAgentHealth, TauriAgentChatTransport } from "../lib/agent";
import { AgentChatControlsProvider } from "./agentChatControls";
import { useAgentSessionStore } from "./agentStore";

export type LatticeAgentProviderProps = {
  workspaceRoot: string | null;
  children: ReactNode;
};

function LatticeAgentRuntimeProvider({
  workspaceRoot,
  children,
}: {
  workspaceRoot: string;
  children: ReactNode;
}) {
  const ensureThreadId = useAgentSessionStore((state) => state.ensureThreadId);
  const recordAgentEvent = useAgentSessionStore((state) => state.recordAgentEvent);
  const setHealthBackend = useAgentSessionStore((state) => state.setHealthBackend);

  const threadId = useMemo(() => ensureThreadId(workspaceRoot), [ensureThreadId, workspaceRoot]);

  const transport = useMemo(
    () =>
      new TauriAgentChatTransport({
        workspaceRoot,
        threadId,
        onAgentEvent: recordAgentEvent,
      }),
    [workspaceRoot, threadId, recordAgentEvent],
  );

  const chat = useChat({
    id: threadId,
    transport: transport as never,
  });

  const runtime = useAISDKRuntime(chat);
  const isStreaming = chat.status === "streaming" || chat.status === "submitted";

  useEffect(() => {
    let cancelled = false;
    const applyHealth = (backend: string | null) => {
      if (!cancelled) {
        setHealthBackend(backend);
      }
    };

    const loadHealth = async () => {
      const delaysMs = [0, 250, 750, 1500];
      for (const delay of delaysMs) {
        if (cancelled) {
          return;
        }
        if (delay > 0) {
          await new Promise((resolve) => setTimeout(resolve, delay));
        }
        try {
          const health = await getAgentHealth();
          if (cancelled) {
            return;
          }
          applyHealth(health.backend);
          return;
        } catch {
          // Daemon/agent plane may not be ready on first paint; retry.
        }
      }
      applyHealth(null);
    };

    void loadHealth();
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot, setHealthBackend]);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <AgentChatControlsProvider value={{ stop: chat.stop, isStreaming }}>
        {children}
      </AgentChatControlsProvider>
    </AssistantRuntimeProvider>
  );
}

export function LatticeAgentProvider({ workspaceRoot, children }: LatticeAgentProviderProps) {
  const root = workspaceRoot?.trim();
  if (!root) {
    return <>{children}</>;
  }

  return <LatticeAgentRuntimeProvider workspaceRoot={root}>{children}</LatticeAgentRuntimeProvider>;
}
