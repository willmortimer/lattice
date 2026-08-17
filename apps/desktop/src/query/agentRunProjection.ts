import { agentEventSchema } from "@lattice/agent-protocol";

import {
  applySpatialAgentEvent,
  type ActiveOverlay,
  type AgentEvidence,
  type TrailStep,
} from "../agent/agentStore";
import type { AgentRunEventDto, AgentRunStatusDto } from "../lib/agentRunEvents";

/** KernelFS lifecycle rows (`run.created` … `run.released` / `run.failed`). */
export type AgentRunLifecycleRow = {
  eventId: string;
  eventSequence: number;
  eventType: string;
  label: string;
  createdAt: number;
  payload: unknown;
  /** Execution/lease id from payload (`kernelfsRunId` or legacy `runId`). */
  kernelfsRunId: string | null;
};

export type AgentRunProjection = {
  runId: string;
  threadId: string | null;
  /** Chat spatial trail folded from `step_*` / overlay payloads. */
  trailSteps: TrailStep[];
  /** Chat spatial evidence folded from `evidence_added` payloads. */
  evidence: AgentEvidence[];
  /** KernelFS execution timeline (`eventType.startsWith("run.")`). */
  lifecycle: AgentRunLifecycleRow[];
  lastSequence: number;
};

export type AgentRunHydrateCursor = {
  runId: string;
  afterSequence: number;
};

type SpatialFoldState = {
  activeOverlays: Record<string, ActiveOverlay>;
  trailSteps: TrailStep[];
  evidence: AgentEvidence[];
  lastEventBackend: string | null;
};

function emptySpatialState(): SpatialFoldState {
  return {
    activeOverlays: {},
    trailSteps: [],
    evidence: [],
    lastEventBackend: null,
  };
}

/** Human label for a KernelFS lifecycle event type. */
export function formatKernelfsLifecycleLabel(eventType: string): string {
  switch (eventType) {
    case "run.created":
      return "Created";
    case "run.hydrating":
      return "Hydrating";
    case "run.ready":
      return "Ready";
    case "run.executing":
      return "Executing";
    case "run.output_available":
      return "Output available";
    case "run.proposal_ready":
      return "Proposal ready";
    case "run.failed":
      return "Failed";
    case "run.released":
      return "Released";
    default:
      return eventType.startsWith("run.")
        ? eventType.slice("run.".length).replace(/_/g, " ")
        : eventType;
  }
}

export function isKernelfsLifecycleEventType(eventType: string): boolean {
  return eventType.startsWith("run.");
}

function kernelfsRunIdFromPayload(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const record = payload as Record<string, unknown>;
  if (typeof record.kernelfsRunId === "string" && record.kernelfsRunId.length > 0) {
    return record.kernelfsRunId;
  }
  if (typeof record.runId === "string" && record.runId.length > 0) {
    return record.runId;
  }
  return null;
}

/**
 * Fold durable `AgentRunEventDto[]` into a workbench view-model.
 *
 * Chat spatial events use underscore types (`step_started`, `evidence_added`);
 * KernelFS lifecycle uses dotted types (`run.created`, …).
 */
export function projectAgentRunEvents(
  events: readonly AgentRunEventDto[],
  options?: { runId?: string; threadId?: string | null; run?: AgentRunStatusDto | null },
): AgentRunProjection {
  const ordered = [...events].sort((a, b) => a.eventSequence - b.eventSequence);
  let spatial = emptySpatialState();
  const lifecycle: AgentRunLifecycleRow[] = [];
  let lastSequence = 0;
  let runId = options?.runId ?? options?.run?.runId ?? ordered[0]?.runId ?? "";
  let threadId =
    options?.threadId ?? options?.run?.threadId ?? ordered[0]?.threadId ?? null;

  for (const event of ordered) {
    lastSequence = Math.max(lastSequence, event.eventSequence);
    if (!runId) {
      runId = event.runId;
    }
    if (!threadId) {
      threadId = event.threadId;
    }

    if (isKernelfsLifecycleEventType(event.eventType)) {
      lifecycle.push({
        eventId: event.id,
        eventSequence: event.eventSequence,
        eventType: event.eventType,
        label: formatKernelfsLifecycleLabel(event.eventType),
        createdAt: event.createdAt,
        payload: event.payload,
        kernelfsRunId: kernelfsRunIdFromPayload(event.payload),
      });
      continue;
    }

    const parsed = agentEventSchema.safeParse(event.payload);
    if (parsed.success) {
      spatial = { ...spatial, ...applySpatialAgentEvent(spatial, parsed.data) };
    }
  }

  return {
    runId,
    threadId,
    trailSteps: spatial.trailSteps,
    evidence: spatial.evidence,
    lifecycle,
    lastSequence,
  };
}

/**
 * Events with `eventSequence > afterSequence`, for one-shot hydrate / live merge.
 */
export function eventsAfterSequence(
  events: readonly AgentRunEventDto[],
  afterSequence: number,
): AgentRunEventDto[] {
  return events.filter((event) => event.eventSequence > afterSequence);
}

/**
 * Sequence cursor after a kill-WebView reconnect.
 *
 * When the live JSONL trail already recorded this chat run, skip replay.
 * Otherwise start at 0 so durable events refill the trail.
 */
export function initialHydrateSequence(
  events: readonly AgentRunEventDto[],
  runLastSequence: number,
  hasLiveTrail: boolean,
): number {
  if (!hasLiveTrail) {
    return 0;
  }
  return Math.max(
    runLastSequence,
    ...events.map((event) => event.eventSequence),
    0,
  );
}

/**
 * Spatial payloads to apply after `afterSequence`, advancing the cursor.
 *
 * KernelFS `run.*` rows are skipped — they belong on `projection.lifecycle`,
 * not the chat trail store.
 */
export function spatialPayloadsAfterSequence(
  events: readonly AgentRunEventDto[],
  afterSequence: number,
): { payloads: unknown[]; nextSequence: number } {
  const pending = eventsAfterSequence(events, afterSequence);
  let nextSequence = afterSequence;
  const payloads: unknown[] = [];
  for (const event of pending) {
    nextSequence = Math.max(nextSequence, event.eventSequence);
    if (!isKernelfsLifecycleEventType(event.eventType)) {
      payloads.push(event.payload);
    }
  }
  return { payloads, nextSequence };
}
