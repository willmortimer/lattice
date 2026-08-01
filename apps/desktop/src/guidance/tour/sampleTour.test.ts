import { describe, expect, it } from "vitest";

import { DEFAULT_GUIDANCE_ANCHOR_IDS } from "../seedAnchors";
import { sampleShellTour } from "./sampleTour";

describe("sampleShellTour", () => {
  it("defines six shell quick-start steps", () => {
    expect(sampleShellTour.steps).toHaveLength(6);
    expect(sampleShellTour.id).toBe("shell.quick-start");
  });

  it("targets registered guidance anchors", () => {
    const anchorIds = new Set<string>(DEFAULT_GUIDANCE_ANCHOR_IDS);
    for (const step of sampleShellTour.steps) {
      expect(anchorIds.has(step.anchor)).toBe(true);
      if (step.fallbackAnchor) {
        expect(anchorIds.has(step.fallbackAnchor)).toBe(true);
      }
    }
  });

  it("includes AI settings and proposal review steps", () => {
    const anchors = sampleShellTour.steps.map((step) => step.anchor);
    expect(anchors).toContain("settings.ai.provider");
    expect(anchors).toContain("agent.proposal.review");
  });
});
