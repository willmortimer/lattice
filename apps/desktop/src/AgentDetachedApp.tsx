import { QueryClientProvider } from "@tanstack/react-query";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  applyAgentDetachedHandoffToSession,
  clearAgentDetachedHandoff,
  readAgentDetachedHandoff,
  refreshAgentDetachedHandoffActiveRun,
  writeAgentDetachedHandoff,
  type AgentDetachedHandoff,
  type AgentDetachedReturnLayout,
} from "./agent/agentDetachedHandoff";
import {
  AGENT_DETACHED_CLOSED_EVENT,
  AGENT_DETACHED_CLOSE_EVENT,
  AGENT_DETACHED_OPEN_EVENT,
  type AgentDetachedClosedPayload,
  type AgentDetachedOpenPayload,
} from "./agent/agentDetachedWindow";
import { AgentHeader } from "./agent/AgentHeader";
import { AgentPanelBody } from "./agent/AgentPanelBody";
import { AgentPanelShell } from "./agent/AgentPanelShell";
import { AgentThread } from "./agent/AgentThread";
import { LatticeAgentProvider } from "./agent/LatticeAgentProvider";
import { useAgentSessionStore } from "./agent/agentStore";
import { useAgentProposalReview } from "./agent/useAgentProposalReview";
import { hasTauri } from "./lib/ipc";
import { queryClient } from "./query/queryClient";
import {
  createDesktopUiStore,
  DesktopUiStoreProvider,
  useDesktopUiStore,
} from "./shell/desktopUiStore";

type DetachedSession = {
  workspaceRoot: string;
  threadId: string;
  returnLayoutMode: AgentDetachedReturnLayout;
};

function seedFromHandoff(handoff: AgentDetachedHandoff): void {
  applyAgentDetachedHandoffToSession(handoff);
  useAgentSessionStore.getState().selectThreadId(handoff.workspaceRoot, handoff.threadId);
}

function AgentDetachedSession({
  session,
  onClose,
}: {
  session: DetachedSession;
  onClose: () => void;
}) {
  const {
    proposalSummaries,
    proposalInboxLoading,
    proposalReview,
    proposalReviewBusy,
    openProposalReview,
    handleProposalAccept,
    handleProposalReject,
    handleProposalCancel,
  } = useAgentProposalReview(session.workspaceRoot);

  return (
    <AgentPanelShell>
      <LatticeAgentProvider workspaceRoot={session.workspaceRoot}>
        <AgentHeader onClose={onClose} workspaceRoot={session.workspaceRoot} />
        <AgentPanelBody
          thread={<AgentThread workspaceRoot={session.workspaceRoot} activeResourcePath={null} />}
          proposals={hasTauri ? proposalSummaries : []}
          proposalLoading={hasTauri ? proposalInboxLoading : false}
          onOpenProposal={hasTauri ? openProposalReview : undefined}
          proposalReview={proposalReview}
          proposalReviewBusy={proposalReviewBusy}
          workspaceRoot={session.workspaceRoot}
          onProposalAccept={(selectedCommandIndices) =>
            void handleProposalAccept(selectedCommandIndices)
          }
          onProposalReject={() => void handleProposalReject()}
          onProposalCancel={handleProposalCancel}
        />
      </LatticeAgentProvider>
    </AgentPanelShell>
  );
}

function AgentDetachedInner() {
  // Workbench body layout inside this window; main keeps layoutMode "detached".
  const setLayoutMode = useDesktopUiStore((state) => state.setAgentLayoutMode);
  const [session, setSession] = useState<DetachedSession | null>(null);
  const sessionRef = useRef<DetachedSession | null>(null);
  sessionRef.current = session;

  const yieldAndHide = useCallback(
    async (returnLayoutMode?: AgentDetachedReturnLayout) => {
      const current = sessionRef.current ?? readAgentDetachedHandoff();
      if (current) {
        const mode = returnLayoutMode ?? current.returnLayoutMode;
        const refreshed = refreshAgentDetachedHandoffActiveRun({
          workspaceRoot: current.workspaceRoot,
          threadId: current.threadId,
          returnLayoutMode: mode,
          activeRun: null,
        });
        writeAgentDetachedHandoff({ ...refreshed, returnLayoutMode: mode });
        await emitTo("main", AGENT_DETACHED_CLOSED_EVENT, {
          returnLayoutMode: mode,
          workspaceRoot: current.workspaceRoot,
          threadId: current.threadId,
        } satisfies AgentDetachedClosedPayload);
      }
      setSession(null);
      await getCurrentWindow().hide();
    },
    [],
  );

  const openSession = useCallback(
    (payload: AgentDetachedOpenPayload) => {
      const fromStorage = readAgentDetachedHandoff();
      const handoff: AgentDetachedHandoff = {
        workspaceRoot: payload.workspaceRoot,
        threadId: payload.threadId,
        returnLayoutMode: payload.returnLayoutMode,
        activeRun: fromStorage?.activeRun ?? null,
      };
      seedFromHandoff(handoff);
      setLayoutMode("workbench");
      setSession({
        workspaceRoot: handoff.workspaceRoot,
        threadId: handoff.threadId,
        returnLayoutMode: handoff.returnLayoutMode,
      });
    },
    [setLayoutMode],
  );

  useEffect(() => {
    const existing = readAgentDetachedHandoff();
    if (existing) {
      openSession({
        workspaceRoot: existing.workspaceRoot,
        threadId: existing.threadId,
        returnLayoutMode: existing.returnLayoutMode,
      });
    }

    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    void listen<AgentDetachedOpenPayload>(AGENT_DETACHED_OPEN_EVENT, (event) => {
      if (!cancelled) {
        openSession(event.payload);
      }
    }).then((stop) => {
      if (cancelled) {
        stop();
        return;
      }
      unlisteners.push(stop);
    });

    void listen(AGENT_DETACHED_CLOSE_EVENT, () => {
      if (!cancelled) {
        void yieldAndHide();
      }
    }).then((stop) => {
      if (cancelled) {
        stop();
        return;
      }
      unlisteners.push(stop);
    });

    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        event.preventDefault();
        await yieldAndHide();
      })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisteners.push(stop);
      });

    return () => {
      cancelled = true;
      for (const stop of unlisteners) {
        stop();
      }
    };
  }, [openSession, yieldAndHide]);

  if (!session) {
    return (
      <div className="agent-detached-shell">
        <div className="agent-detached-native-titlebar" data-tauri-drag-region />
        <div className="agent-detached-placeholder" role="status">
          <p>Waiting for workspace agent…</p>
        </div>
      </div>
    );
  }

  return (
    <div className="agent-detached-shell">
      <div className="agent-detached-native-titlebar" data-tauri-drag-region />
      <AgentDetachedSession
        key={session.workspaceRoot}
        session={session}
        onClose={() => {
          void yieldAndHide();
        }}
      />
    </div>
  );
}

export function AgentDetachedApp() {
  const [store] = useState(() => {
    const next = createDesktopUiStore();
    next.getState().setAgentLayoutMode("workbench");
    next.getState().setAgentPanelOpen(true);
    return next;
  });

  useEffect(() => {
    return () => {
      clearAgentDetachedHandoff();
    };
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <DesktopUiStoreProvider store={store}>
        <AgentDetachedInner />
      </DesktopUiStoreProvider>
    </QueryClientProvider>
  );
}
