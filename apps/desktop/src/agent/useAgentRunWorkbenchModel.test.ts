import { describe, expect, it } from "vitest";

import type { AgentRunEventDto } from "../lib/agentRunEvents";
import {
  initialHydrateSequence,
  projectAgentRunEvents,
  spatialPayloadsAfterSequence,
} from "../query/agentRunProjection";

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

describe("useAgentRunWorkbenchModel hydrate helpers", () => {
  it("projects joined chat + KernelFS events and hydrates spatial trail once", () => {
    const events: AgentRunEventDto[] = [
      event({
        id: "c1",
        eventSequence: 1,
        eventType: "step_started",
        payload: {
          type: "step_started",
          runId: "run-chat-1",
          stepId: "s1",
          kind: "tool",
          label: "run_wasi_guest",
        },
      }),
      event({
        id: "k1",
        eventSequence: 2,
        eventType: "run.created",
        payload: {
          type: "run.created",
          runId: "run-kf-1",
          kernelfsRunId: "run-kf-1",
          chatRunId: "run-chat-1",
        },
      }),
      event({
        id: "k2",
        eventSequence: 3,
        eventType: "run.released",
        payload: {
          type: "run.released",
          runId: "run-kf-1",
          kernelfsRunId: "run-kf-1",
          chatRunId: "run-chat-1",
        },
      }),
      event({
        id: "c2",
        eventSequence: 4,
        eventType: "step_completed",
        payload: {
          type: "step_completed",
          runId: "run-chat-1",
          stepId: "s1",
          durationMs: 9,
          summary: "done",
        },
      }),
    ];

    const projection = projectAgentRunEvents(events, { runId: "run-chat-1" });
    expect(projection.trailSteps).toHaveLength(1);
    expect(projection.lifecycle).toHaveLength(2);
    expect(projection.lifecycle[0]?.kernelfsRunId).toBe("run-kf-1");

    let cursor = initialHydrateSequence(events, 4, false);
    const first = spatialPayloadsAfterSequence(events, cursor);
    expect(first.payloads).toHaveLength(2);
    cursor = first.nextSequence;

    const second = spatialPayloadsAfterSequence(events, cursor);
    expect(second.payloads).toEqual([]);
  });
});
