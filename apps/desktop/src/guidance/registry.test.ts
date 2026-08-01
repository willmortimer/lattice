// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";

import { createDomGuidanceAnchor } from "./domAnchor";
import {
  clearGuidanceAnchors,
  getGuidanceAnchor,
  listGuidanceAnchors,
  registerGuidanceAnchor,
} from "./registry";
import {
  createDefaultGuidanceAnchors,
  DEFAULT_GUIDANCE_ANCHOR_IDS,
  seedGuidanceAnchors,
} from "./seedAnchors";

describe("guidance anchor registry", () => {
  afterEach(() => {
    clearGuidanceAnchors();
    document.body.innerHTML = "";
  });

  it("registers, looks up, and unregisters anchors by id", () => {
    const anchor = createDomGuidanceAnchor({ id: "test.anchor" });
    const unregister = registerGuidanceAnchor(anchor);
    expect(getGuidanceAnchor("test.anchor")).toBe(anchor);
    unregister();
    expect(getGuidanceAnchor("test.anchor")).toBeUndefined();
  });

  it("lists registered anchors", () => {
    const first = createDomGuidanceAnchor({ id: "a" });
    const second = createDomGuidanceAnchor({ id: "b" });
    registerGuidanceAnchor(first);
    registerGuidanceAnchor(second);
    expect(listGuidanceAnchors()).toEqual([first, second]);
  });

  it("seeds the default semantic anchor catalog", () => {
    const unseed = seedGuidanceAnchors();
    expect(listGuidanceAnchors().map((anchor) => anchor.id)).toEqual([
      ...DEFAULT_GUIDANCE_ANCHOR_IDS,
    ]);
    expect(createDefaultGuidanceAnchors()).toHaveLength(8);
    unseed();
    expect(listGuidanceAnchors()).toEqual([]);
  });

  it("resolves DOM anchors via data-guidance-anchor", () => {
    const button = document.createElement("button");
    button.setAttribute("data-guidance-anchor", "shell.search");
    button.getBoundingClientRect = () =>
      new DOMRect(10, 20, 100, 30) as DOMRect;
    document.body.append(button);

    const anchor = createDomGuidanceAnchor({
      id: "shell.search",
      describe: "Search",
    });
    expect(anchor.isAvailable()).toBe(true);
    expect(anchor.getRect()).not.toBeNull();
    expect(anchor.describe?.()).toBe("Search");
  });
});
