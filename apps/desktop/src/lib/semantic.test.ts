import { describe, expect, it } from "vitest";

import {
  SEMANTIC_MODEL_CONFIRM,
  semanticStatusLabel,
  type SemanticStatusState,
} from "./semantic";

describe("semanticStatusLabel", () => {
  it("maps each lifecycle state", () => {
    const states: SemanticStatusState[] = [
      "stopped",
      "downloading",
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
    expect(semanticStatusLabel("downloading", null, 42)).toBe("Downloading 42%");
    expect(semanticStatusLabel("indexing", 4)).toBe("Indexing (4 pending)");
    expect(semanticStatusLabel("ready", 0)).toBe("Ready");
  });

  it("confirm copy mentions size and license", () => {
    expect(SEMANTIC_MODEL_CONFIRM).toContain("~640 MB");
    expect(SEMANTIC_MODEL_CONFIRM).toContain("Apache-2.0");
    expect(SEMANTIC_MODEL_CONFIRM).toContain("never uploaded");
  });
});
