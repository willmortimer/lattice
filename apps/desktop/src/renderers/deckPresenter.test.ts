// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import { deckTransitionAttr, humanizeSlideId, renderDeckNotesHtml } from "./deckPresenter";

describe("deckTransitionAttr", () => {
  it("collapses every transition to cut under reduced motion", () => {
    expect(deckTransitionAttr({ type: "fade" }, true)).toBe("cut");
    expect(deckTransitionAttr({ type: "push", direction: "up" }, true)).toBe("cut");
  });

  it("encodes push direction for CSS hooks", () => {
    expect(deckTransitionAttr({ type: "push", direction: "left" }, false)).toBe("push-left");
    expect(deckTransitionAttr({ type: "push", direction: "right" }, false)).toBe("push-right");
    expect(deckTransitionAttr({ type: "push", direction: "up" }, false)).toBe("push-up");
    expect(deckTransitionAttr({ type: "push", direction: "down" }, false)).toBe("push-down");
    expect(deckTransitionAttr({ type: "push" }, false)).toBe("push-left");
  });

  it("passes fade and cut through unchanged", () => {
    expect(deckTransitionAttr({ type: "fade" }, false)).toBe("fade");
    expect(deckTransitionAttr({ type: "cut" }, false)).toBe("cut");
    expect(deckTransitionAttr(null, false)).toBe("cut");
  });
});

describe("humanizeSlideId", () => {
  it("title-cases kebab and snake ids", () => {
    expect(humanizeSlideId("title")).toBe("Title");
    expect(humanizeSlideId("go-to-market")).toBe("Go To Market");
    expect(humanizeSlideId("ask_slide")).toBe("Ask Slide");
  });
});

describe("renderDeckNotesHtml", () => {
  it("returns empty for blank notes", () => {
    expect(renderDeckNotesHtml(null)).toBe("");
    expect(renderDeckNotesHtml("   ")).toBe("");
  });

  it("renders markdown structure without executing raw HTML", () => {
    const html = renderDeckNotesHtml("# Title\n\nSay **this**, then pause.\n\n<script>alert(1)</script>");
    expect(html).toContain("<h1>");
    expect(html).toContain("<strong>this</strong>");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });
});
