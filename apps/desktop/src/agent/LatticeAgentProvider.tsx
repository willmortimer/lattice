import { useChat } from "@ai-sdk/react";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { useAISDKRuntime } from "@assistant-ui/react-ai-sdk";
import { useEffect, useMemo, useRef, type ReactNode } from "react";

import { getAgentHealth, TauriAgentChatTransport } from "../lib/agent";
import {
  getAgentThread,
  uiMessagesFromThreadMessages,
} from "../lib/agentThreads";
import { getCloudSessionStatus, isCloudAiEntitled } from "../lib/cloud";
import { loadProfile } from "../lib/profile";
import { hasOpenaiApiKey } from "../lib/openaiKey";
import { resolveAgentDefaultsFromAiSettings } from "./agentAiDefaults";
import { AgentChatControlsProvider } from "./agentChatControls";
import { isAgentComposerDisabled, useAgentSessionStore } from "./agentStore";

export type LatticeAgentProviderProps = {
  workspaceRoot: string | null;
  children: ReactNode;
};

/**
 * Ensures a workspace thread id exists without updating the store during render.
 * Updating zustand inside render trips React #185 (maximum update depth).
 */
function LatticeAgentThreadGate({
  workspaceRoot,
  children,
}: {
  workspaceRoot: string;
  children: (threadId: string) => ReactNode;
}) {
  const ensureThreadId = useAgentSessionStore((state) => state.ensureThreadId);
  const activeThreadId = useAgentSessionStore((state) => state.threadIds[workspaceRoot]);

  useEffect(() => {
    ensureThreadId(workspaceRoot);
  }, [workspaceRoot, ensureThreadId]);

  if (!activeThreadId) {
    return (
      <div className="agent-thread-placeholder" role="status">
        <p>Starting agent…</p>
      </div>
    );
  }

  return <>{children(activeThreadId)}</>;
}

function LatticeAgentRuntimeProvider({
  workspaceRoot,
  threadId,
  children,
}: {
  workspaceRoot: string;
  threadId: string;
  children: ReactNode;
}) {
  const recordAgentEvent = useAgentSessionStore((state) => state.recordAgentEvent);
  const setHealthSnapshot = useAgentSessionStore((state) => state.setHealthSnapshot);
  const applyProfileAiDefaults = useAgentSessionStore((state) => state.applyProfileAiDefaults);
  const setByoOpenaiKeyPresent = useAgentSessionStore((state) => state.setByoOpenaiKeyPresent);
  const bumpThreadListEpoch = useAgentSessionStore((state) => state.bumpThreadListEpoch);
  const aiMode = useAgentSessionStore((state) => state.aiMode);

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
        const signedIn = cloudStatus?.signedIn === true;
        applyProfileAiDefaults(
          resolveAgentDefaultsFromAiSettings(profile.settings.desktop.ai, {
            cloudSignedIn: signedIn,
            cloudAiEntitled: cloudStatus ? isCloudAiEntitled(cloudStatus) : false,
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

  useEffect(() => {
    let cancelled = false;
    if (aiMode !== "byoOpenai") {
      setByoOpenaiKeyPresent(null);
      return () => {
        cancelled = true;
      };
    }

    void hasOpenaiApiKey()
      .then((present) => {
        if (!cancelled) {
          setByoOpenaiKeyPresent(present);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setByoOpenaiKeyPresent(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [aiMode, setByoOpenaiKeyPresent]);

  const transport = useMemo(
    () =>
      new TauriAgentChatTransport({
        workspaceRoot,
        threadId,
        onAgentEvent: recordAgentEvent,
        resolveRunOptions: () => {
          const state = useAgentSessionStore.getState();
          if (isAgentComposerDisabled(state)) {
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

  const hydratedThreadRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    hydratedThreadRef.current = null;

    const loadTranscript = async () => {
      try {
        const result = await getAgentThread({ workspaceRoot, threadId });
        if (cancelled) {
          return;
        }
        chat.setMessages(uiMessagesFromThreadMessages(result.messages) as never);
        hydratedThreadRef.current = threadId;
      } catch {
        if (cancelled) {
          return;
        }
        // Fresh / not-yet-persisted threads start empty.
        chat.setMessages([]);
        hydratedThreadRef.current = threadId;
      }
    };

    void loadTranscript();
    return () => {
      cancelled = true;
    };
    // Reload only when the active thread changes; chat.setMessages is stable enough.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspaceRoot, threadId]);

  const previousStatusRef = useRef(chat.status);
  useEffect(() => {
    const previous = previousStatusRef.current;
    previousStatusRef.current = chat.status;
    if (
      (previous === "streaming" || previous === "submitted") &&
      chat.status === "ready"
    ) {
      bumpThreadListEpoch();
    }
  }, [chat.status, bumpThreadListEpoch]);

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

  return (
    <LatticeAgentThreadGate workspaceRoot={root}>
      {(threadId) => (
        <LatticeAgentRuntimeProvider workspaceRoot={root} threadId={threadId}>
          {children}
        </LatticeAgentRuntimeProvider>
      )}
    </LatticeAgentThreadGate>
  );
}
