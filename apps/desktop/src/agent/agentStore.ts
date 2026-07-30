import {
  agentEventSchema,
  type AgentEvent,
  type AgentStepKind,
  type EvidenceAddedEvent,
  type OverlayPurpose,
  type OverlayShowEvent,
  type WorkspaceAnchor,
} from "@lattice/agent-protocol";
import { create } from "zustand";

import type { AiMode } from "../lib/profile";
import type { AgentAiDefaults } from "./agentAiDefaults";
import {
  defaultModelForProvider,
  type SelectableAgentProvider,
} from "./modelCatalog";
import { isAgentProviderKind } from "./providerKind";

const SELECTION_STORAGE_KEY = "lattice.agent.selection.v1";

function readStoredSelection(): {
  selectedProvider: SelectableAgentProvider | null;
  selectedModel: string | null;
} {
  if (typeof sessionStorage === "undefined") {
    return { selectedProvider: null, selectedModel: null };
  }
  try {
    const raw = sessionStorage.getItem(SELECTION_STORAGE_KEY);
    if (!raw) {
      return { selectedProvider: null, selectedModel: null };
    }
    const parsed = JSON.parse(raw) as {
      selectedProvider?: string;
      selectedModel?: string;
    };
    const provider =
      parsed.selectedProvider === "openai" || parsed.selectedProvider === "pioneer"
        ? parsed.selectedProvider
        : null;
    const model =
      typeof parsed.selectedModel === "string" && parsed.selectedModel.trim()
        ? parsed.selectedModel.trim()
        : null;
    return { selectedProvider: provider, selectedModel: model };
  } catch {
    return { selectedProvider: null, selectedModel: null };
  }
}

function persistSelection(
  selectedProvider: SelectableAgentProvider | null,
  selectedModel: string | null,
): void {
  if (typeof sessionStorage === "undefined") {
    return;
  }
  try {
    sessionStorage.setItem(
      SELECTION_STORAGE_KEY,
      JSON.stringify({ selectedProvider, selectedModel }),
    );
  } catch {
    // Ignore quota / private mode failures.
  }
}

export type AgentFollowMode = "guide" | "quiet";

export type ActiveOverlay = {
  overlayId: string;
  runId: string;
  anchors: WorkspaceAnchor[];
  purpose: OverlayPurpose;
  commentary?: string;
};

export type TrailReplaySnapshot = {
  anchors: WorkspaceAnchor[];
  overlayId: string;
  purpose: OverlayPurpose;
  commentary?: string;
};

export type TrailStep = {
  stepId: string;
  runId: string;
  kind: AgentStepKind;
  label: string;
  status: "in_progress" | "completed";
  durationMs?: number;
  summary?: string;
  /** Snapshot for click-to-replay after overlays clear. */
  anchors?: WorkspaceAnchor[];
  overlayId?: string;
  purpose?: OverlayPurpose;
  commentary?: string;
};

export type AgentEvidence = {
  evidenceId: string;
  runId: string;
  resourceId: string;
  path: string;
  revision?: string;
  excerpt: string;
  anchor?: WorkspaceAnchor;
  score?: number;
};

const MAX_TRAIL_LABELS = 20;
const MAX_TRAIL_STEPS = 50;
const MAX_EVIDENCE = 100;

export function shouldRevealViewport(followMode: AgentFollowMode): boolean {
  return followMode === "guide";
}

function resolveProviderFromProfileAndHealth(
  state: Pick<AgentSessionStore, "selectedProvider" | "accountAiDisabled" | "aiMode">,
  backend: string | null,
): SelectableAgentProvider | null {
  if (state.selectedProvider) {
    return state.selectedProvider;
  }
  if (state.accountAiDisabled) {
    return null;
  }
  if (state.aiMode === "byoOpenai") {
    return "openai";
  }
  if (backend === "openai" || backend === "pioneer") {
    return backend;
  }
  return null;
}

function resolveModelFromProfileAndHealth(
  state: Pick<AgentSessionStore, "selectedModel" | "accountAiDisabled">,
  healthModel: string | null,
  provider: SelectableAgentProvider | null,
): string | null {
  if (state.selectedModel) {
    return state.selectedModel;
  }
  if (state.accountAiDisabled) {
    return null;
  }
  if (healthModel) {
    return healthModel;
  }
  return provider ? defaultModelForProvider(provider) : null;
}

type AgentSessionStore = {
  threadIds: Record<string, string>;
  healthBackend: string | null;
  healthModel: string | null;
  healthOk: boolean | null;
  healthDegraded: boolean | null;
  lastEventBackend: string | null;
  aiMode: AiMode | null;
  accountAiDisabled: boolean;
  selectedProvider: SelectableAgentProvider | null;
  selectedModel: string | null;
  trailLabels: string[];
  followMode: AgentFollowMode;
  activeOverlays: Record<string, ActiveOverlay>;
  trailSteps: TrailStep[];
  evidence: AgentEvidence[];
  ensureThreadId: (workspaceRoot: string) => string;
  setHealthBackend: (backend: string | null) => void;
  setHealthSnapshot: (snapshot: {
    backend: string | null;
    model?: string | null;
    ok?: boolean | null;
    degraded?: boolean | null;
  }) => void;
  applyProfileAiDefaults: (defaults: AgentAiDefaults) => void;
  setSelectedProvider: (provider: SelectableAgentProvider) => void;
  setSelectedModel: (model: string) => void;
  setFollowMode: (mode: AgentFollowMode) => void;
  consumeEvent: (event: AgentEvent) => void;
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

function overlayFromShow(event: OverlayShowEvent): ActiveOverlay {
  return {
    overlayId: event.overlayId,
    runId: event.runId,
    anchors: event.anchors,
    purpose: event.purpose,
    ...(event.commentary !== undefined ? { commentary: event.commentary } : {}),
  };
}

function evidenceFromEvent(event: EvidenceAddedEvent): AgentEvidence {
  return {
    evidenceId: event.evidenceId,
    runId: event.runId,
    resourceId: event.resourceId,
    path: event.path,
    excerpt: event.excerpt,
    ...(event.revision !== undefined ? { revision: event.revision } : {}),
    ...(event.anchor !== undefined ? { anchor: event.anchor } : {}),
    ...(event.score !== undefined ? { score: event.score } : {}),
  };
}

function replaySnapshotFromOverlay(event: OverlayShowEvent): TrailReplaySnapshot {
  return {
    anchors: event.anchors,
    overlayId: event.overlayId,
    purpose: event.purpose,
    ...(event.commentary !== undefined ? { commentary: event.commentary } : {}),
  };
}

function attachReplayToTrailStep(
  step: TrailStep,
  snapshot: TrailReplaySnapshot,
): TrailStep {
  return {
    ...step,
    anchors: snapshot.anchors,
    overlayId: snapshot.overlayId,
    purpose: snapshot.purpose,
    ...(snapshot.commentary !== undefined ? { commentary: snapshot.commentary } : {}),
  };
}

function isReplayableTrailKind(kind: AgentStepKind): boolean {
  return kind === "navigation" || kind === "search" || kind === "tool";
}

export function applyOverlayShowToTrailSteps(
  trailSteps: TrailStep[],
  event: OverlayShowEvent,
): TrailStep[] {
  const snapshot = replaySnapshotFromOverlay(event);
  let attachedInProgress = false;

  const updated = trailSteps.map((step) => {
    if (
      step.runId === event.runId &&
      step.status === "in_progress" &&
      !attachedInProgress
    ) {
      attachedInProgress = true;
      return attachReplayToTrailStep(step, snapshot);
    }
    return step;
  });

  if (attachedInProgress) {
    for (let index = updated.length - 1; index >= 0; index -= 1) {
      const step = updated[index]!;
      if (
        step.runId === event.runId &&
        step.kind === "navigation" &&
        step.status === "completed" &&
        !step.anchors?.length
      ) {
        const next = [...updated];
        next[index] = attachReplayToTrailStep(step, snapshot);
        return next;
      }
    }
    return updated;
  }

  for (let index = updated.length - 1; index >= 0; index -= 1) {
    const step = updated[index]!;
    if (
      step.runId === event.runId &&
      isReplayableTrailKind(step.kind) &&
      !step.anchors?.length
    ) {
      const next = [...updated];
      next[index] = attachReplayToTrailStep(step, snapshot);
      return next;
    }
  }

  return updated;
}

export function applySpatialAgentEvent(
  state: Pick<
    AgentSessionStore,
    "activeOverlays" | "trailSteps" | "evidence" | "lastEventBackend"
  >,
  event: AgentEvent,
): Pick<AgentSessionStore, "activeOverlays" | "trailSteps" | "evidence" | "lastEventBackend"> {
  switch (event.type) {
    case "overlay_show": {
      const overlay = overlayFromShow(event);
      return {
        ...state,
        activeOverlays: {
          ...state.activeOverlays,
          [overlay.overlayId]: overlay,
        },
        trailSteps: applyOverlayShowToTrailSteps(state.trailSteps, event),
      };
    }
    case "overlay_clear": {
      if (event.overlayId === undefined) {
        const nextOverlays = { ...state.activeOverlays };
        for (const [overlayId, overlay] of Object.entries(nextOverlays)) {
          if (overlay.runId === event.runId) {
            delete nextOverlays[overlayId];
          }
        }
        return { ...state, activeOverlays: nextOverlays };
      }

      const { [event.overlayId]: _removed, ...remaining } = state.activeOverlays;
      return { ...state, activeOverlays: remaining };
    }
    case "step_started": {
      const nextStep: TrailStep = {
        stepId: event.stepId,
        runId: event.runId,
        kind: event.kind,
        label: event.label,
        status: "in_progress",
      };
      const withoutDuplicate = state.trailSteps.filter(
        (step) => !(step.stepId === event.stepId && step.runId === event.runId),
      );
      return {
        ...state,
        trailSteps: [...withoutDuplicate, nextStep].slice(-MAX_TRAIL_STEPS),
      };
    }
    case "step_completed": {
      let found = false;
      const trailSteps = state.trailSteps.map((step) => {
        if (step.stepId === event.stepId && step.runId === event.runId) {
          found = true;
          return {
            ...step,
            status: "completed" as const,
            durationMs: event.durationMs,
            ...(event.summary !== undefined ? { summary: event.summary } : {}),
          };
        }
        return step;
      });

      if (!found) {
        const synthesized: TrailStep = {
          stepId: event.stepId,
          runId: event.runId,
          kind: "execution",
          label: event.summary ?? event.stepId,
          status: "completed",
          durationMs: event.durationMs,
          ...(event.summary !== undefined ? { summary: event.summary } : {}),
        };
        return {
          ...state,
          trailSteps: [...trailSteps, synthesized].slice(-MAX_TRAIL_STEPS),
        };
      }

      return { ...state, trailSteps };
    }
    case "evidence_added": {
      const entry = evidenceFromEvent(event);
      const withoutDuplicate = state.evidence.filter(
        (item) => !(item.evidenceId === entry.evidenceId && item.runId === entry.runId),
      );
      return {
        ...state,
        evidence: [...withoutDuplicate, entry].slice(-MAX_EVIDENCE),
      };
    }
    case "run_started": {
      const backend =
        event.provider && isAgentProviderKind(event.provider)
          ? event.provider.toLowerCase()
          : state.lastEventBackend;
      return backend === state.lastEventBackend
        ? state
        : { ...state, lastEventBackend: backend };
    }
    default:
      return state;
  }
}

export const initialAgentSessionState = {
  threadIds: {},
  healthBackend: null as string | null,
  healthModel: null as string | null,
  healthOk: null as boolean | null,
  healthDegraded: null as boolean | null,
  lastEventBackend: null as string | null,
  aiMode: null as AiMode | null,
  accountAiDisabled: false,
  ...readStoredSelection(),
  trailLabels: [] as string[],
  followMode: "guide" as const,
  activeOverlays: {},
  trailSteps: [] as TrailStep[],
  evidence: [] as AgentEvidence[],
};

export const useAgentSessionStore = create<AgentSessionStore>((set, get) => ({
  ...initialAgentSessionState,
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
  setHealthSnapshot: (snapshot) =>
    set((state) => {
      const backend = snapshot.backend;
      const model =
        snapshot.model === undefined
          ? state.healthModel
          : snapshot.model && snapshot.model.trim()
            ? snapshot.model.trim()
            : null;
      const nextProvider = resolveProviderFromProfileAndHealth(state, backend);
      const nextModel = resolveModelFromProfileAndHealth(state, model, nextProvider);
      if (nextProvider !== state.selectedProvider || nextModel !== state.selectedModel) {
        persistSelection(nextProvider, nextModel);
      }
      return {
        healthBackend: backend,
        healthModel: model,
        healthOk: snapshot.ok === undefined ? state.healthOk : snapshot.ok,
        healthDegraded:
          snapshot.degraded === undefined ? state.healthDegraded : snapshot.degraded,
        selectedProvider: nextProvider,
        selectedModel: nextModel,
      };
    }),
  applyProfileAiDefaults: (defaults) =>
    set(() => {
      const stored = readStoredSelection();
      const hasStoredSelection =
        stored.selectedProvider !== null || stored.selectedModel !== null;
      const base = {
        aiMode: defaults.aiMode,
        accountAiDisabled: defaults.accountAiDisabled,
      };
      if (hasStoredSelection) {
        return base;
      }
      const provider = defaults.provider;
      const model =
        defaults.model ?? (provider ? defaultModelForProvider(provider) : null);
      if (provider && model) {
        persistSelection(provider, model);
      }
      return {
        ...base,
        selectedProvider: provider,
        selectedModel: model,
      };
    }),
  setSelectedProvider: (provider) =>
    set(() => {
      const model = defaultModelForProvider(provider);
      persistSelection(provider, model);
      return { selectedProvider: provider, selectedModel: model };
    }),
  setSelectedModel: (model) =>
    set((state) => {
      const trimmed = model.trim();
      persistSelection(state.selectedProvider, trimmed);
      return { selectedModel: trimmed };
    }),
  setFollowMode: (mode) => set({ followMode: mode }),
  consumeEvent: (event) => {
    set((state) => applySpatialAgentEvent(state, event));
  },
  recordAgentEvent: (event) => {
    const label = extractEventLabel(event);
    const backend = extractEventBackend(event);
    const parsed = agentEventSchema.safeParse(event);

    set((state) => {
      let next = state;

      if (parsed.success) {
        next = { ...next, ...applySpatialAgentEvent(next, parsed.data) };
      } else if (backend) {
        next = { ...next, lastEventBackend: backend };
      }

      if (!label) {
        return next;
      }

      return {
        ...next,
        trailLabels: [...next.trailLabels.slice(-(MAX_TRAIL_LABELS - 1)), label],
      };
    });
  },
}));
