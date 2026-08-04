import { describe, expect, it } from "vitest";
import * as Y from "yjs";

import {
  createAnchorFromTypeIndex,
  resolveAnchorAbsoluteIndex,
} from "./commentAnchors";
import {
  createStickyComment,
  listStickyComments,
  setStickyCommentResolved,
} from "./commentStore";

function seedTextDoc(initial: string): { ydoc: Y.Doc; text: Y.Text } {
  const ydoc = new Y.Doc();
  const text = ydoc.getText("body");
  text.insert(0, initial);
  return { ydoc, text };
}

describe("sticky comment anchors", () => {
  it("relative position survives insert-before", () => {
    const { ydoc, text } = seedTextDoc("Hello world");
    // Anchor at the start of "world"
    const anchor = createAnchorFromTypeIndex(text, 6);
    expect(resolveAnchorAbsoluteIndex(ydoc, anchor)).toBe(6);

    text.insert(0, "PREFIX ");
    expect(text.toString()).toBe("PREFIX Hello world");
    expect(resolveAnchorAbsoluteIndex(ydoc, anchor)).toBe(13);
    expect(text.toString().slice(13)).toBe("world");
  });

  it("serialize and restore comments from Y.Doc update", () => {
    const { ydoc, text } = seedTextDoc("Hello sticky comments");
    const anchorStart = createAnchorFromTypeIndex(text, 6);
    const anchorEnd = createAnchorFromTypeIndex(text, 12);

    createStickyComment(ydoc, {
      id: "cmt-1",
      body: "Looks good",
      author: "Editor 1",
      createdAt: 1_700_000_000_000,
      anchors: {
        anchorStart,
        anchorEnd,
        quote: "sticky",
      },
    });

    const update = Y.encodeStateAsUpdate(ydoc);
    const restored = new Y.Doc();
    Y.applyUpdate(restored, update);

    const comments = listStickyComments(restored);
    expect(comments).toHaveLength(1);
    expect(comments[0]).toMatchObject({
      id: "cmt-1",
      body: "Looks good",
      author: "Editor 1",
      resolved: false,
      quote: "sticky",
      createdAt: 1_700_000_000_000,
    });

    const restoredText = restored.getText("body");
    expect(resolveAnchorAbsoluteIndex(restored, comments[0]!.anchorStart)).toBe(6);
    expect(restoredText.toString().slice(6, 12)).toBe("sticky");

    restoredText.insert(0, ">>> ");
    expect(resolveAnchorAbsoluteIndex(restored, comments[0]!.anchorStart)).toBe(10);
  });

  it("resolve and unresolve updates the Y.Map entry", () => {
    const { ydoc, text } = seedTextDoc("abc");
    createStickyComment(ydoc, {
      id: "cmt-2",
      body: "thread",
      author: "Editor 2",
      anchors: {
        anchorStart: createAnchorFromTypeIndex(text, 0),
        anchorEnd: createAnchorFromTypeIndex(text, 3),
        quote: "abc",
      },
    });

    expect(setStickyCommentResolved(ydoc, "cmt-2", true)).toBe(true);
    expect(listStickyComments(ydoc)[0]?.resolved).toBe(true);
    expect(setStickyCommentResolved(ydoc, "cmt-2", false)).toBe(true);
    expect(listStickyComments(ydoc)[0]?.resolved).toBe(false);
  });
});
