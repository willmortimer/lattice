import { useChat } from "@ai-sdk/react";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { useAISDKRuntime } from "@assistant-ui/react-ai-sdk";
import { useEffect, useMemo, type ReactNode } from "react";

import { getAgentHealth, TauriAgentChatTransport } from "../lib/agent";
import { getCloudSessionStatus } from "../lib/cloud";
import { loadProfile } from "../lib/profile";
import { resolveAgentDefaultsFromAiSettings } from "./agentAiDefaults";
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
  const setHealthSnapshot = useAgentSessionStore((state) => state.setHealthSnapshot);
  const applyProfileAiDefaults = useAgentSessionStore((state) => state.applyProfileAiDefaults);

  const threadId = useMemo(() => ensureThreadId(workspaceRoot), [ensureThreadId, workspaceRoot]);

  useEffect(() => {
    let cancelled = false;
    const applyDefaults = async () => {
      try {
        const [profile, cloudStatus] = await Promise.all([
          loadProfile(),
          getCloudSessionStatus().catch(() => null),
        ]);
        if (cancelled) {
          return;
        }
        applyProfileAiDefaults(
          resolveAgentDefaultsFromAiSettings(profile.settings.desktop.ai, {
            cloudSignedIn: cloudStatus?.signedIn === true,
          }),
        );
      } catch {
        // Profile load failure should not block the agent shell.
      }
    };
    void applyDefaults();
    return () => {
      cancelled = true;
    };
  }, [applyProfileAiDefaults]);

  const transport = useMemo(
    () =>
      new TauriAgentChatTransport({
        workspaceRoot,
        threadId,
        onAgentEvent: recordAgentEvent,
        resolveRunOptions: () => {
          const state = useAgentSessionStore.getState();
          if (state.accountAiDisabled) {
            return {};
          }
          return {
            provider: state.selectedProvider ?? undefined,
            model: state.selectedModel ?? undefined,
          };
        },
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
    const applyHealth = (snapshot: {
      backend: string | null;
      model?: string | null;
      ok?: boolean | null;
      degraded?: boolean | null;
    }) => {
      if (!cancelled) {
        setHealthSnapshot(snapshot);
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
          applyHealth({
            backend: health.backend,
            model: health.model ?? null,
            ok: health.ok,
            degraded: health.degraded,
          });
          return;
        } catch {
          // Daemon/agent plane may not be ready on first paint; retry.
        }
      }
      applyHealth({ backend: null, model: null, ok: null, degraded: null });
    };

    void loadHealth();
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot, setHealthSnapshot]);

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
