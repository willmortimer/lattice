import { afterEach, describe, expect, it, vi } from "vitest";

import type { ActiveOverlay } from "./agentStore";
import {
  applyActiveOverlays,
  clearOverlayHighlights,
  revealBehaviorForFollowMode,
} from "./agentOverlayEffects";
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

function createOverlay(overrides: Partial<ActiveOverlay> = {}): ActiveOverlay {
  return {
    overlayId: "overlay-1",
    runId: "run-1",
    anchors: [markdownAnchor],
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

describe("revealBehaviorForFollowMode", () => {
  it("uses reveal in guide mode", () => {
    expect(revealBehaviorForFollowMode("guide")).toBe("reveal");
  });

  it("uses peek in quiet mode", () => {
    expect(revealBehaviorForFollowMode("quiet")).toBe("peek");
  });
});

describe("applyActiveOverlays", () => {
  afterEach(() => {
    clearAnchorAdapters();
  });

  it("reveals and highlights anchors in guide mode", () => {
    const adapter = createAdapter();
    registerAnchorAdapter(adapter);

    const clears = applyActiveOverlays(
      { "overlay-1": createOverlay() },
      "guide",
    );

    expect(adapter.reveal).toHaveBeenCalledWith(markdownAnchor, "reveal");
    expect(adapter.highlight).toHaveBeenCalledWith(markdownAnchor, {
      overlayId: "overlay-1",
      purpose: "attention",
    });
    expect(clears).toHaveLength(1);
  });

  it("highlights without forcing reveal in quiet mode", () => {
    const adapter = createAdapter();
    registerAnchorAdapter(adapter);

    applyActiveOverlays({ "overlay-1": createOverlay() }, "quiet");

    expect(adapter.reveal).toHaveBeenCalledWith(markdownAnchor, "peek");
    expect(adapter.highlight).toHaveBeenCalled();
  });

  it("invokes highlight clear functions when overlays are cleared", () => {
    const adapter = createAdapter();
    registerAnchorAdapter(adapter);

    const clears = applyActiveOverlays(
      { "overlay-1": createOverlay() },
      "guide",
    );
    clearOverlayHighlights(clears);

    expect(adapter.clearHighlight).toHaveBeenCalledTimes(1);
  });

  it("skips anchors without a registered adapter", () => {
    const clears = applyActiveOverlays(
      { "overlay-1": createOverlay() },
      "guide",
    );

    expect(clears).toEqual([]);
  });
});
