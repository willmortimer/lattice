// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { createDomGuidanceAnchor } from "./domAnchor";
import {
  getAnchorRectForAriaLabel,
  resolveGuidanceAnchorFromAriaLabel,
  resolveGuidanceAnchorIdFromAriaLabel,
} from "./demoBridge";
import { clearGuidanceAnchors, registerGuidanceAnchor } from "./registry";

describe("guidance demo bridge", () => {
  afterEach(() => {
    clearGuidanceAnchors();
    document.body.innerHTML = "";
  });

  it("maps known aria labels to guidance anchor ids", () => {
    const anchor = createDomGuidanceAnchor({ id: "resource-tree.new-page" });
    registerGuidanceAnchor(anchor);

    expect(resolveGuidanceAnchorIdFromAriaLabel("Create resource")).toBe(
      "resource-tree.new-page",
    );
    expect(resolveGuidanceAnchorFromAriaLabel("Create resource")).toBe(anchor);
  });

  it("falls back to aria-labeled elements that declare data-guidance-anchor", () => {
    const button = document.createElement("button");
    button.setAttribute("aria-label", "Custom search");
    button.setAttribute("data-guidance-anchor", "shell.search");
    button.getBoundingClientRect = () => new DOMRect(4, 8, 120, 24) as DOMRect;
    document.body.append(button);

    const anchor = createDomGuidanceAnchor({ id: "shell.search" });
    registerGuidanceAnchor(anchor);

    expect(resolveGuidanceAnchorIdFromAriaLabel("Custom search")).toBe("shell.search");
    expect(getAnchorRectForAriaLabel("Custom search")).toEqual(expect.objectContaining({ width: 120 }));
  });
});
