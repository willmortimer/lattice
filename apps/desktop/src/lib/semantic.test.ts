import { describe, expect, it } from "vitest";

import { semanticStatusLabel, type SemanticStatusState } from "./semantic";

describe("semanticStatusLabel", () => {
  it("maps each lifecycle state", () => {
    const states: SemanticStatusState[] = [
      "stopped",
      "preparing",
      "indexing",
      "ready",
      "degraded",
      "failed",
    ];
    for (const state of states) {
      expect(semanticStatusLabel(state, null).length).toBeGreaterThan(0);
    }
    expect(semanticStatusLabel("stopped", null)).toBe("Not prepared");
    expect(semanticStatusLabel("indexing", 4)).toBe("Indexing (4 pending)");
    expect(semanticStatusLabel("ready", 0)).toBe("Ready");
  });
});
