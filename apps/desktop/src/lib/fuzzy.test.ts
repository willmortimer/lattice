import { describe, expect, it } from "vitest";

import { fuzzyFilter, fuzzyMatch } from "./fuzzy";

describe("fuzzyMatch", () => {
  it("matches a subsequence regardless of case", () => {
    expect(fuzzyMatch("cp", "Command Palette")).not.toBeNull();
    expect(fuzzyMatch("CP", "command palette")).not.toBeNull();
  });

  it("returns null when the pattern's characters are out of order", () => {
    expect(fuzzyMatch("pc", "Command Palette")).toBeNull();
  });

  it("returns null when a character is missing entirely", () => {
    expect(fuzzyMatch("cpz", "Command Palette")).toBeNull();
  });

  it("matches an empty pattern against anything with a zero score", () => {
    expect(fuzzyMatch("", "anything")).toEqual({ score: 0, indices: [] });
  });

  it("scores a word-boundary match higher than a mid-word match at the same skip distance", () => {
    // Both land "b" one character past "a", so the skip-distance penalty
    // is identical; only the boundary before "b" differs (a "/" versus a
    // plain letter).
    const boundaryMatch = fuzzyMatch("ab", "a/bc");
    const midWordMatch = fuzzyMatch("ab", "axbc");
    expect(boundaryMatch!.score).toBeGreaterThan(midWordMatch!.score);
  });

  it("scores consecutive characters higher than scattered ones", () => {
    const consecutive = fuzzyMatch("co", "Command");
    const scattered = fuzzyMatch("cd", "Command");
    expect(consecutive!.score).toBeGreaterThan(scattered!.score);
  });
});

describe("fuzzyFilter", () => {
  const items = ["Roadmap.md", "Research/Competitor Analysis.md", "Product/Vision.md"];

  it("returns every item, unscored, for an empty pattern", () => {
    const results = fuzzyFilter(items, "", (item) => item);
    expect(results.map((r) => r.item)).toEqual(items);
    expect(results.every((r) => r.score === 0)).toBe(true);
  });

  it("filters out items that don't match", () => {
    const results = fuzzyFilter(items, "zzz", (item) => item);
    expect(results).toEqual([]);
  });

  it("ranks the best match first", () => {
    const results = fuzzyFilter(items, "vision", (item) => item);
    expect(results[0]?.item).toBe("Product/Vision.md");
  });

  it("matches against a derived text, not just the item itself", () => {
    const results = fuzzyFilter(
      [{ label: "Vision", hint: "Product/Vision.md" }],
      "product",
      (item) => `${item.label} ${item.hint}`,
    );
    expect(results).toHaveLength(1);
  });
});
