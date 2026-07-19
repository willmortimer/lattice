import { describe, expect, it } from "vitest";

import { classifyEditorHref } from "./linkClick";

describe("classifyEditorHref", () => {
  it("routes wiki links into the workspace resolver", () => {
    expect(classifyEditorHref("wiki:Product%2FVision")).toEqual({
      kind: "workspace",
      target: "Product/Vision",
    });
  });

  it("routes relative and root-relative markdown paths into the workspace resolver", () => {
    expect(classifyEditorHref("./architecture.md")).toEqual({
      kind: "workspace",
      target: "./architecture.md",
    });
    expect(classifyEditorHref("../31-open-questions-and-decision-register.md")).toEqual({
      kind: "workspace",
      target: "../31-open-questions-and-decision-register.md",
    });
    expect(classifyEditorHref("docs/voice/README.md#setup")).toEqual({
      kind: "workspace",
      target: "docs/voice/README.md#setup",
    });
  });

  it("keeps http(s)/mailto/tel external", () => {
    expect(classifyEditorHref("https://example.com/docs")).toEqual({
      kind: "external",
      url: "https://example.com/docs",
    });
    expect(classifyEditorHref("mailto:hi@example.com")).toEqual({
      kind: "external",
      url: "mailto:hi@example.com",
    });
  });

  it("treats in-document fragments separately", () => {
    expect(classifyEditorHref("#milestones")).toEqual({
      kind: "fragment",
      hash: "milestones",
    });
  });

  it("ignores non-workspace schemes", () => {
    expect(classifyEditorHref("javascript:alert(1)")).toEqual({ kind: "ignored" });
    expect(classifyEditorHref("asset://localhost/foo")).toEqual({ kind: "ignored" });
    expect(classifyEditorHref("")).toEqual({ kind: "ignored" });
  });
});
