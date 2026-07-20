import { describe, expect, it } from "vitest";

import {
  looksFtsOnlyWhileSemanticEnabled,
  searchHitBadgeKind,
  searchHitBadgeLabel,
} from "./searchHitBadge";

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
