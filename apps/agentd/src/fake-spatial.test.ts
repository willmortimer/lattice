import { describe, expect, it } from "vitest";

import type { AgentEvent } from "@lattice/agent-protocol";

import {
  emitFakeSpatialSequence,
  FAKE_SPATIAL_PROMPT,
  isFakeSpatialPrompt,
} from "./fake-spatial.js";

describe("isFakeSpatialPrompt", () => {
  it("matches the documented smoke prompt", () => {
    expect(isFakeSpatialPrompt(FAKE_SPATIAL_PROMPT)).toBe(true);
    expect(isFakeSpatialPrompt("  SPATIAL-DEMO  ")).toBe(true);
    expect(isFakeSpatialPrompt("hello")).toBe(false);
  });
});

describe("emitFakeSpatialSequence", () => {
  it("emits search step, overlay_show, and step_completed", () => {
    const events: AgentEvent[] = [];

    emitFakeSpatialSequence("run-1", (event) => {
      events.push(event);
    });

    expect(events.map((event) => event.type)).toEqual([
      "step_started",
      "overlay_show",
      "step_completed",
    ]);
    expect(events[0]).toMatchObject({
      type: "step_started",
      kind: "search",
      stepId: "fake-spatial-step",
    });
    expect(events[1]).toMatchObject({
      type: "overlay_show",
      overlayId: "fake-spatial-overlay",
      anchors: [
        {
          kind: "markdown-block",
          resourceId: "fake-demo-page",
          blockId: "fake-demo-block",
        },
      ],
    });
    expect(events[2]).toMatchObject({
      type: "step_completed",
      stepId: "fake-spatial-step",
      summary: "Highlighted demo block",
    });
  });
});
