import { describe, expect, it } from "vitest";

import type { AgentRunEventDto } from "../lib/agentRunEvents";
import {
  eventsAfterSequence,
  formatKernelfsLifecycleLabel,
  isKernelfsLifecycleEventType,
  projectAgentRunEvents,
} from "./agentRunProjection";
import { queryKeys } from "./keys";

function event(
  partial: Partial<AgentRunEventDto> &
    Pick<AgentRunEventDto, "id" | "eventSequence" | "eventType">,
): AgentRunEventDto {
  return {
    runId: "run-chat-1",
    threadId: "thread-1",
    payload: {},
    createdAt: partial.eventSequence * 1000,
    ...partial,
  };
}

describe("agentRunProjection", () => {
  it("folds chat spatial step/evidence events and KernelFS lifecycle separately", () => {
    const projection = projectAgentRunEvents([
      event({
        id: "c1",
        eventSequence: 1,
        eventType: "step_started",
        payload: {
          type: "step_started",
          runId: "run-chat-1",
          stepId: "s1",
          kind: "search",
          label: "Search notes",
        },
      }),
      event({
        id: "k1",
        eventSequence: 2,
        eventType: "run.created",
        runId: "run-kf-1",
        payload: { type: "run.created", runId: "run-kf-1" },
      }),
      event({
        id: "c2",
        eventSequence: 3,
        eventType: "evidence_added",
        payload: {
          type: "evidence_added",
          runId: "run-chat-1",
          evidenceId: "e1",
          resourceId: "r1",
          path: "docs/a.md",
          excerpt: "hello",
        },
      }),
      event({
        id: "k2",
        eventSequence: 4,
        eventType: "run.executing",
        runId: "run-kf-1",
        payload: { type: "run.executing", runId: "run-kf-1" },
      }),
      event({
        id: "c3",
        eventSequence: 5,
        eventType: "step_completed",
        payload: {
          type: "step_completed",
          runId: "run-chat-1",
          stepId: "s1",
          durationMs: 12,
          summary: "Found 1",
        },
      }),
      event({
        id: "k3",
        eventSequence: 6,
        eventType: "run.released",
        runId: "run-kf-1",
        payload: { type: "run.released", runId: "run-kf-1" },
      }),
    ]);

    expect(projection.trailSteps).toEqual([
      {
        stepId: "s1",
        runId: "run-chat-1",
        kind: "search",
        label: "Search notes",
        status: "completed",
        durationMs: 12,
        summary: "Found 1",
      },
    ]);
    expect(projection.evidence).toEqual([
      {
        evidenceId: "e1",
        runId: "run-chat-1",
        resourceId: "r1",
        path: "docs/a.md",
        excerpt: "hello",
      },
    ]);
    expect(projection.lifecycle.map((row) => row.eventType)).toEqual([
      "run.created",
      "run.executing",
      "run.released",
    ]);
    expect(projection.lifecycle[0]?.label).toBe("Created");
    expect(projection.lastSequence).toBe(6);
  });

  it("does not treat chat run_completed as KernelFS lifecycle", () => {
    const projection = projectAgentRunEvents([
      event({
        id: "done",
        eventSequence: 1,
        eventType: "run_completed",
        payload: { type: "run_completed", runId: "run-chat-1" },
      }),
    ]);
    expect(projection.lifecycle).toEqual([]);
    expect(isKernelfsLifecycleEventType("run_completed")).toBe(false);
    expect(isKernelfsLifecycleEventType("run.failed")).toBe(true);
  });

  it("filters events after a sequence cursor", () => {
    const events = [
      event({ id: "a", eventSequence: 1, eventType: "run.created" }),
      event({ id: "b", eventSequence: 2, eventType: "run.ready" }),
      event({ id: "c", eventSequence: 3, eventType: "run.released" }),
    ];
    expect(eventsAfterSequence(events, 1).map((row) => row.id)).toEqual(["b", "c"]);
  });

  it("formats known KernelFS lifecycle labels", () => {
    expect(formatKernelfsLifecycleLabel("run.output_available")).toBe("Output available");
    expect(formatKernelfsLifecycleLabel("run.proposal_ready")).toBe("Proposal ready");
  });
});

describe("agent run query keys", () => {
  it("exposes status, events, active, and kernelfsRun keys", () => {
    expect(queryKeys.agentRunStatus("/ws", "run-1")).toEqual([
      "agent-run-status",
      "/ws",
      "run-1",
    ]);
    expect(queryKeys.agentRunEvents("/ws", "run-1")).toEqual([
      "agent-run-events",
      "/ws",
      "run-1",
    ]);
    expect(queryKeys.agentRunActive("/ws", "thread-1")).toEqual([
      "agent-run-active",
      "/ws",
      "thread-1",
    ]);
    expect(queryKeys.kernelfsRun("run-kf-1")).toEqual(["kernelfs-run", "run-kf-1"]);
  });
});
