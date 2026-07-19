import { describe, expect, it } from "vitest";

import { mapFenceLanguage, PLAIN_FENCE_LANGUAGE } from "./mapFenceLanguage";

describe("mapFenceLanguage", () => {
  it("maps typescript aliases", () => {
    expect(mapFenceLanguage("ts")).toBe("typescript");
    expect(mapFenceLanguage("typescript")).toBe("typescript");
    expect(mapFenceLanguage("TS")).toBe("typescript");
  });

  it("maps javascript aliases", () => {
    expect(mapFenceLanguage("js")).toBe("javascript");
    expect(mapFenceLanguage("javascript")).toBe("javascript");
  });

  it("maps rust, python, json, yaml, bash, and sql aliases", () => {
    expect(mapFenceLanguage("rust")).toBe("rust");
    expect(mapFenceLanguage("py")).toBe("python");
    expect(mapFenceLanguage("python")).toBe("python");
    expect(mapFenceLanguage("json")).toBe("json");
    expect(mapFenceLanguage("yaml")).toBe("yaml");
    expect(mapFenceLanguage("yml")).toBe("yaml");
    expect(mapFenceLanguage("bash")).toBe("bash");
    expect(mapFenceLanguage("sh")).toBe("bash");
    expect(mapFenceLanguage("shell")).toBe("bash");
    expect(mapFenceLanguage("sql")).toBe("sql");
  });

  it("falls back to plain text for empty, missing, or unknown languages", () => {
    expect(mapFenceLanguage(null)).toBe(PLAIN_FENCE_LANGUAGE);
    expect(mapFenceLanguage(undefined)).toBe(PLAIN_FENCE_LANGUAGE);
    expect(mapFenceLanguage("")).toBe(PLAIN_FENCE_LANGUAGE);
    expect(mapFenceLanguage("   ")).toBe(PLAIN_FENCE_LANGUAGE);
    expect(mapFenceLanguage("cobol")).toBe(PLAIN_FENCE_LANGUAGE);
    expect(mapFenceLanguage("mermaid")).toBe(PLAIN_FENCE_LANGUAGE);
  });
});
