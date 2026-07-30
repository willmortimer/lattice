import { describe, expect, it } from "vitest";

import {
  formatFusedScore,
  formatSearchHitScore,
  looksFtsOnlyWhileSemanticEnabled,
  SEARCH_VECTORS_BEHIND_BANNER,
  searchHitBadgeKind,
  searchHitBadgeLabel,
  shouldShowVectorsBehindBanner,
} from "./searchHitBadge";
import { VECTORS_BEHIND_MESSAGE } from "./semantic";

describe("searchHitBadgeKind", () => {
  it("returns null for FTS-only hits", () => {
    expect(searchHitBadgeKind({})).toBeNull();
    expect(searchHitBadgeKind({ lexicalRank: null, semanticRank: null })).toBeNull();
  });

  it("maps rank presence to badge kinds", () => {
    expect(searchHitBadgeKind({ lexicalRank: 1 })).toBe("keyword");
    expect(searchHitBadgeKind({ semanticRank: 2 })).toBe("semantic");
    expect(searchHitBadgeKind({ lexicalRank: 1, semanticRank: 2 })).toBe("both");
  });
});

describe("searchHitBadgeLabel", () => {
  it("labels each badge kind", () => {
    expect(searchHitBadgeLabel("keyword")).toBe("Keyword");
    expect(searchHitBadgeLabel("semantic")).toBe("Semantic");
    expect(searchHitBadgeLabel("both")).toBe("Both");
  });
});

describe("looksFtsOnlyWhileSemanticEnabled", () => {
  it("is false when semantic search is off or there are no hits", () => {
    expect(looksFtsOnlyWhileSemanticEnabled(false, [{ path: "a.md", title: "a", snippet: null, rank: 1 }])).toBe(
      false,
    );
    expect(looksFtsOnlyWhileSemanticEnabled(true, [])).toBe(false);
  });

  it("is true when semantic is on and no hit has a semantic rank", () => {
    expect(
      looksFtsOnlyWhileSemanticEnabled(true, [
        { path: "a.md", title: "a", snippet: null, rank: 1, lexicalRank: 1 },
      ]),
    ).toBe(true);
  });

  it("is false once any hit has a semantic rank", () => {
    expect(
      looksFtsOnlyWhileSemanticEnabled(true, [
        { path: "a.md", title: "a", snippet: null, rank: 1, semanticRank: 1 },
      ]),
    ).toBe(false);
  });
});

describe("formatFusedScore", () => {
  it("formats typical RRF scores compactly", () => {
    expect(formatFusedScore(0.016393442)).toBe("0.0164");
    expect(formatFusedScore(0.032786885)).toBe("0.0328");
  });

  it("handles larger scores", () => {
    expect(formatFusedScore(1.23456)).toBe("1.23");
    expect(formatFusedScore(0.456)).toBe("0.456");
  });
});

describe("formatSearchHitScore", () => {
  it("returns null for FTS-only hits", () => {
    expect(formatSearchHitScore({})).toBeNull();
  });

  it("shows fused score and per-list ranks", () => {
    expect(
      formatSearchHitScore({
        fusedScore: 0.032786885,
        lexicalRank: 1,
        semanticRank: 3,
      }),
    ).toBe("0.0328 K1·S3");
  });

  it("shows ranks without fused score", () => {
    expect(formatSearchHitScore({ lexicalRank: 2, semanticRank: 1 })).toBe("K2·S1");
  });
});

describe("shouldShowVectorsBehindBanner", () => {
  it("is false when semantic search is off or status is missing", () => {
    expect(shouldShowVectorsBehindBanner(false, null)).toBe(false);
    expect(
      shouldShowVectorsBehindBanner(true, {
        state: "ready",
        pendingChunks: 0,
        message: null,
      }),
    ).toBe(false);
  });

  it("is true when semantic is on and vectors lag workspace", () => {
    expect(
      shouldShowVectorsBehindBanner(true, {
        state: "indexing",
        pendingChunks: 0,
        message: VECTORS_BEHIND_MESSAGE,
      }),
    ).toBe(true);
  });

  it("is false while chunks are still pending", () => {
    expect(
      shouldShowVectorsBehindBanner(true, {
        state: "indexing",
        pendingChunks: 2,
        message: VECTORS_BEHIND_MESSAGE,
      }),
    ).toBe(false);
  });
});

describe("SEARCH_VECTORS_BEHIND_BANNER", () => {
  it("mentions keywords", () => {
    expect(SEARCH_VECTORS_BEHIND_BANNER.toLowerCase()).toContain("keyword");
  });
});
