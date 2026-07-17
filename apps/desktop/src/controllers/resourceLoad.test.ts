import { describe, expect, it } from "vitest";
import { canEditTextResource, isTextFormatId, MAX_EDITABLE_TEXT_BYTES } from "./resourceLoad";
import type { ResourceInspection } from "../lib/resourceRuntime";

function inspection(size: number, overrides: Partial<ResourceInspection["capabilities"]> = {}): ResourceInspection {
  return {
    path: "notes.txt",
    kind: "file",
    profile: "plain-text",
    capabilities: {
      canInspect: true,
      canReadRange: true,
      canReadTextWindow: true,
      canUpdate: true,
      isText: true,
      isBinary: false,
      validatesStructure: false,
      maxEditBytes: 0,
      ...overrides,
    },
    revision: "rev:1",
    size,
    isDirectory: false,
    encoding: "utf8",
    probeBytes: 0,
    diagnostics: [],
  };
}

describe("resourceLoad helpers", () => {
  it("recognizes native and derived text format IDs", () => {
    expect(isTextFormatId("plain-text")).toBe(true);
    expect(isTextFormatId("file:json")).toBe(true);
    expect(isTextFormatId("file:image")).toBe(false);
  });

  it("treats files above the editable threshold as read-only windows", () => {
    expect(canEditTextResource(inspection(MAX_EDITABLE_TEXT_BYTES))).toBe(true);
    expect(canEditTextResource(inspection(MAX_EDITABLE_TEXT_BYTES + 1))).toBe(false);
    expect(canEditTextResource(inspection(10, { isText: false }))).toBe(false);
    expect(canEditTextResource(inspection(10, { canUpdate: false }))).toBe(false);
  });
});
