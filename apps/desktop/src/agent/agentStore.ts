import { create } from "zustand";

import { isAgentProviderKind } from "./providerKind";

type AgentSessionStore = {
  threadIds: Record<string, string>;
  healthBackend: string | null;
  lastEventBackend: string | null;
  trailLabels: string[];
  ensureThreadId: (workspaceRoot: string) => string;
  setHealthBackend: (backend: string | null) => void;
  recordAgentEvent: (event: unknown) => void;
};

function extractEventLabel(event: unknown): string | null {
  if (typeof event !== "object" || event === null) {
    return null;
  }
  if ("type" in event && typeof event.type === "string") {
    return event.type;
  }
  return null;
}

function extractEventBackend(event: unknown): string | null {
  if (typeof event !== "object" || event === null) {
    return null;
  }
  if ("provider" in event && typeof event.provider === "string") {
    return isAgentProviderKind(event.provider) ? event.provider.toLowerCase() : null;
  }
  if ("backend" in event && typeof event.backend === "string") {
    return isAgentProviderKind(event.backend) ? event.backend.toLowerCase() : null;
  }
  return null;
}

export const useAgentSessionStore = create<AgentSessionStore>((set, get) => ({
  threadIds: {},
  healthBackend: null,
  lastEventBackend: null,
  trailLabels: [],
  ensureThreadId: (workspaceRoot) => {
    const existing = get().threadIds[workspaceRoot];
    if (existing) {
      return existing;
    }
    const threadId = crypto.randomUUID();
    set((state) => ({
      threadIds: { ...state.threadIds, [workspaceRoot]: threadId },
    }));
    return threadId;
  },
  setHealthBackend: (backend) => set({ healthBackend: backend }),
  recordAgentEvent: (event) => {
    const label = extractEventLabel(event);
    const backend = extractEventBackend(event);
    set((state) => ({
      trailLabels: label ? [...state.trailLabels.slice(-19), label] : state.trailLabels,
      ...(backend ? { lastEventBackend: backend } : {}),
    }));
  },
}));
