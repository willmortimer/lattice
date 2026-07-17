import { describe, expect, it } from "vitest";
import { parseStructuredSource } from "./structuredParserCore";

describe("parseStructuredSource", () => {
  it("parses valid JSON into a bounded tree", () => {
    const result = parseStructuredSource('{"items":[{"id":1}]}', "json");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.root.kind).toBe("object");
  });

  it("keeps malformed JSON as a diagnostic without throwing", () => {
    const result = parseStructuredSource("{not-json", "json");
    expect(result.ok).toBe(false);
    expect(result.diagnostics.length).toBeGreaterThan(0);
  });

  it("rejects YAML that exceeds the depth limit", () => {
    const deep = "a:\n  b:\n    c:\n      d:\n        e:\n          f:\n            g:\n              h:\n                value: 1";
    const result = parseStructuredSource(deep, "yaml", { maxDepth: 8 });
    expect(result.ok).toBe(false);
    expect(result.diagnostics[0]?.message).toMatch(/depth/i);
  });

  it("does not expand YAML aliases into resolved values", () => {
    const result = parseStructuredSource("anchor: &ref\nchild: *ref", "yaml");
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.root.kind).toBe("object");
    const child = result.root.kind === "object" ? result.root.entries.find((entry) => entry.key === "child") : undefined;
    expect(child?.value.kind).toBe("alias");
  });
});
