import { describe, expect, it } from "vitest";

import {
  isSemanticPackPrepared,
  isVectorsBehindStatus,
  SEMANTIC_MODEL_CONFIRM,
  semanticProviderLabel,
  semanticStatusLabel,
  VECTORS_BEHIND_EXPLANATION,
  VECTORS_BEHIND_MESSAGE,
  type SemanticStatus,
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
    expect(semanticStatusLabel("indexing", 0)).toBe("Indexing…");
    expect(semanticStatusLabel("ready", 0)).toBe("Ready");
  });

  it("labels vectors-behind indexing distinctly", () => {
    expect(
      semanticStatusLabel("indexing", 0, null, VECTORS_BEHIND_MESSAGE),
    ).toBe("Vectors behind workspace");
    expect(
      semanticStatusLabel("indexing", null, null, "Vectors behind workspace"),
    ).toBe("Vectors behind workspace");
    // Pending work stays a normal indexing label even if message is present.
    expect(
      semanticStatusLabel("indexing", 3, null, VECTORS_BEHIND_MESSAGE),
    ).toBe("Indexing (3 pending)");
  });

  it("confirm copy mentions size and license", () => {
    expect(SEMANTIC_MODEL_CONFIRM).toContain("~640 MB");
    expect(SEMANTIC_MODEL_CONFIRM).toContain("Apache-2.0");
    expect(SEMANTIC_MODEL_CONFIRM).toContain("never uploaded");
  });
});

describe("isVectorsBehindStatus", () => {
  it("detects runtime stale-vector message on idle indexing", () => {
    expect(
      isVectorsBehindStatus({
        state: "indexing",
        message: VECTORS_BEHIND_MESSAGE,
        pendingChunks: 0,
      }),
    ).toBe(true);
    expect(
      isVectorsBehindStatus({
        state: "indexing",
        message: "Vectors Behind Workspace",
        pendingChunks: null,
      }),
    ).toBe(true);
    expect(
      isVectorsBehindStatus({
        state: "indexing",
        message: VECTORS_BEHIND_MESSAGE,
        pendingChunks: 2,
      }),
    ).toBe(false);
    expect(
      isVectorsBehindStatus({
        state: "ready",
        message: VECTORS_BEHIND_MESSAGE,
        pendingChunks: 0,
      }),
    ).toBe(false);
    expect(
      isVectorsBehindStatus({
        state: "indexing",
        message: null,
        pendingChunks: 0,
      }),
    ).toBe(false);
  });

  it("exposes user-facing explanation copy", () => {
    expect(VECTORS_BEHIND_EXPLANATION.length).toBeGreaterThan(20);
    expect(VECTORS_BEHIND_EXPLANATION.toLowerCase()).toContain("keyword");
  });
});

describe("isSemanticPackPrepared", () => {
  it("treats non-stopped non-failed states as prepared", () => {
    expect(isSemanticPackPrepared(null)).toBe(false);
    expect(isSemanticPackPrepared(baseStatus({ state: "stopped" }))).toBe(false);
    expect(isSemanticPackPrepared(baseStatus({ state: "failed" }))).toBe(false);
    expect(isSemanticPackPrepared(baseStatus({ state: "ready" }))).toBe(true);
    expect(isSemanticPackPrepared(baseStatus({ state: "indexing" }))).toBe(true);
    expect(isSemanticPackPrepared(baseStatus({ state: "downloading" }))).toBe(true);
  });
});

describe("semanticProviderLabel", () => {
  it("formats provider · model · dimensions", () => {
    expect(
      semanticProviderLabel({
        providerId: "llama.cpp",
        modelId: "Qwen3-Embedding-0.6B",
        dimensions: 512,
      }),
    ).toBe("llama.cpp · Qwen3-Embedding-0.6B · 512-d");
  });

  it("returns null when no identity fields", () => {
    expect(semanticProviderLabel({})).toBeNull();
    expect(
      semanticProviderLabel({
        providerId: null,
        modelId: null,
        dimensions: null,
      }),
    ).toBeNull();
  });
});

function baseStatus(overrides: Partial<SemanticStatus> = {}): SemanticStatus {
  return {
    state: "stopped",
    pendingChunks: null,
    message: null,
    progressPercent: null,
    providerId: null,
    modelId: null,
    dimensions: null,
    ...overrides,
  };
}
