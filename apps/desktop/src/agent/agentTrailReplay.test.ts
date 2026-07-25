import { afterEach, describe, expect, it, vi } from "vitest";

import {
  applyActiveOverlays,
  clearOverlayHighlights,
} from "./agentOverlayEffects";
import {
  canReplayTrailStep,
  overlayFromTrailStep,
  replayTrailStep,
} from "./agentTrailReplay";
import type { TrailStep } from "./agentStore";
import {
  clearAnchorAdapters,
  registerAnchorAdapter,
} from "./adapters/registry";
import type { AgentAnchorAdapter } from "./adapters/types";

const markdownAnchor = {
  kind: "markdown-block" as const,
  resourceId: "Notes/Page.md",
  blockId: "root|paragraph#0",
};

function createTrailStep(overrides: Partial<TrailStep> = {}): TrailStep {
  return {
    stepId: "step-1",
    runId: "run-1",
    kind: "search",
    label: "Search demo page",
    status: "completed",
    anchors: [markdownAnchor],
    overlayId: "overlay-1",
    purpose: "attention",
    ...overrides,
  };
}

function createAdapter(): AgentAnchorAdapter & {
  reveal: ReturnType<typeof vi.fn>;
  highlight: ReturnType<typeof vi.fn>;
  clearHighlight: ReturnType<typeof vi.fn>;
} {
  const clearHighlight = vi.fn();
  const highlight = vi.fn(() => clearHighlight);
  const reveal = vi.fn(async () => undefined);

  return {
    kind: "markdown-block",
    resourceId: "Notes/Page.md",
    reveal,
    highlight,
    clearHighlight,
  };
}

describe("canReplayTrailStep", () => {
  it("returns true when anchors are present", () => {
    expect(canReplayTrailStep(createTrailStep())).toBe(true);
  });

  it("returns false without anchors", () => {
    expect(canReplayTrailStep(createTrailStep({ anchors: undefined }))).toBe(false);
    expect(canReplayTrailStep(createTrailStep({ anchors: [] }))).toBe(false);
  });
});

describe("overlayFromTrailStep", () => {
  it("builds an active overlay snapshot", () => {
    expect(overlayFromTrailStep(createTrailStep())).toEqual({
      overlayId: "overlay-1",
      runId: "run-1",
      anchors: [markdownAnchor],
      purpose: "attention",
    });
  });

  it("falls back to a replay overlay id", () => {
    expect(
      overlayFromTrailStep(createTrailStep({ overlayId: undefined }))?.overlayId,
    ).toBe("replay-step-1");
  });
});

describe("replayTrailStep", () => {
  afterEach(() => {
    clearAnchorAdapters();
  });

  it("reveals and highlights anchors through adapters", () => {
    const adapter = createAdapter();
    registerAnchorAdapter(adapter);

    const clears = replayTrailStep(createTrailStep(), "guide");

    expect(adapter.reveal).toHaveBeenCalledWith(markdownAnchor, "reveal");
    expect(adapter.highlight).toHaveBeenCalledWith(markdownAnchor, {
      overlayId: "overlay-1",
      purpose: "attention",
    });
    expect(clears).toHaveLength(1);

    clearOverlayHighlights(clears);
    expect(adapter.clearHighlight).toHaveBeenCalledTimes(1);
  });

  it("uses peek behavior in quiet mode", () => {
    const adapter = createAdapter();
    registerAnchorAdapter(adapter);

    replayTrailStep(createTrailStep(), "quiet");

    expect(adapter.reveal).toHaveBeenCalledWith(markdownAnchor, "peek");
    expect(adapter.highlight).toHaveBeenCalled();
  });

  it("returns no clears when the step is not replayable", () => {
    const clears = replayTrailStep(createTrailStep({ anchors: undefined }), "guide");
    expect(clears).toEqual([]);
  });

  it("reuses applyActiveOverlays for the overlay snapshot", () => {
    const adapter = createAdapter();
    registerAnchorAdapter(adapter);
    const overlay = overlayFromTrailStep(createTrailStep());
    expect(overlay).not.toBeNull();

    const clears = applyActiveOverlays(
      { [overlay!.overlayId]: overlay! },
      "guide",
    );

    expect(adapter.reveal).toHaveBeenCalled();
    expect(clears).toHaveLength(1);
  });
});
