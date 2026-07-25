/** Deterministic spatial event sequence for fake provider smoke tests. */

import type { AgentEvent } from "@lattice/agent-protocol";

import type { EventSink } from "./runner.js";

/** Prompt that triggers the hermetic spatial fixture in fake provider/backends. */
export const FAKE_SPATIAL_PROMPT = "spatial-demo";

export function isFakeSpatialPrompt(prompt: string): boolean {
  return prompt.trim().toLowerCase() === FAKE_SPATIAL_PROMPT;
}

const FAKE_SPATIAL_STEP_ID = "fake-spatial-step";
const FAKE_SPATIAL_OVERLAY_ID = "fake-spatial-overlay";

export function emitFakeSpatialSequence(runId: string, sink: EventSink): void {
  const startedAt = Date.now();
  const anchor = {
    kind: "markdown-block" as const,
    resourceId: "fake-demo-page",
    blockId: "fake-demo-block",
  };

  sink({
    type: "step_started",
    runId,
    stepId: FAKE_SPATIAL_STEP_ID,
    kind: "search",
    label: "Search demo page",
  } satisfies AgentEvent);
  sink({
    type: "overlay_show",
    runId,
    overlayId: FAKE_SPATIAL_OVERLAY_ID,
    anchors: [anchor],
    purpose: "attention",
    commentary: "Fake spatial fixture",
  } satisfies AgentEvent);
  sink({
    type: "step_completed",
    runId,
    stepId: FAKE_SPATIAL_STEP_ID,
    durationMs: Math.max(1, Date.now() - startedAt),
    summary: "Highlighted demo block",
  } satisfies AgentEvent);
}
