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
    void getAgentHealth()
      .then((health) => {
        if (!cancelled) {
          setHealthBackend(health.backend);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setHealthBackend(null);
        }
      });
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
