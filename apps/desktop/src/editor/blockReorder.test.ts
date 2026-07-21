import { getSchema } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { EditorState } from "@tiptap/pm/state";
import { describe, expect, it } from "vitest";

import {
  isBlockDragArmed,
  reorderBlockTransaction,
  resolveBlockDragFromPos,
  topLevelBlockPos,
} from "./blockReorder";

const schema = getSchema([StarterKit]);

function createDocState(paragraphs: string[]) {
  return EditorState.create({
    schema,
    doc: schema.node(
      "doc",
      null,
      paragraphs.map((text) => schema.node("paragraph", null, [schema.text(text)])),
    ),
  });
}

function docTexts(state: EditorState): string[] {
  const texts: string[] = [];
  state.doc.forEach((node) => {
    texts.push(node.textContent);
  });
  return texts;
}

describe("resolveBlockDragFromPos", () => {
  it("falls back to module-level pos when MIME and plain are empty (WKWebView)", () => {
    expect(resolveBlockDragFromPos(undefined, undefined, 42)).toBe(42);
    expect(resolveBlockDragFromPos("", "", 7)).toBe(7);
  });

  it("prefers MIME over plain and module-level pos", () => {
    expect(resolveBlockDragFromPos("10", "20", 30)).toBe(10);
    expect(resolveBlockDragFromPos(undefined, "20", 30)).toBe(20);
  });

  it("returns null when nothing is available", () => {
    expect(resolveBlockDragFromPos(undefined, undefined, null)).toBeNull();
    expect(resolveBlockDragFromPos("", "", null)).toBeNull();
  });
});

describe("isBlockDragArmed", () => {
  it("arms dragover from module-level pos without MIME types", () => {
    expect(isBlockDragArmed(null)).toBe(false);
    expect(isBlockDragArmed(0)).toBe(true);
    expect(isBlockDragArmed(12)).toBe(true);
  });
});

describe("topLevelBlockPos", () => {
  it("resolves a position inside a paragraph to that block start", () => {
    const state = createDocState(["alpha", "beta", "gamma"]);
    // First block starts at 0; text content starts at 1.
    expect(topLevelBlockPos(state.doc, 1)).toBe(0);
    // Second block: size of first paragraph is 2 + text length = 7 ("alpha").
    const secondStart = state.doc.child(0).nodeSize;
    expect(topLevelBlockPos(state.doc, secondStart + 1)).toBe(secondStart);
  });
});

describe("reorderBlockTransaction", () => {
  it("moves the first block after the second", () => {
    let state = createDocState(["alpha", "beta", "gamma"]);
    const fromPos = 0;
    const firstSize = state.doc.child(0).nodeSize;
    const secondSize = state.doc.child(1).nodeSize;
    // Drop onto the start of the third block → alpha should land after beta.
    const toPos = firstSize + secondSize;

    const tr = reorderBlockTransaction(state, fromPos, toPos);
    expect(tr).not.toBeNull();
    state = state.apply(tr!);
    expect(docTexts(state)).toEqual(["beta", "alpha", "gamma"]);
  });

  it("moves the last block to the top", () => {
    let state = createDocState(["alpha", "beta", "gamma"]);
    const fromPos = state.doc.child(0).nodeSize + state.doc.child(1).nodeSize;
    const tr = reorderBlockTransaction(state, fromPos, 0);
    expect(tr).not.toBeNull();
    state = state.apply(tr!);
    expect(docTexts(state)).toEqual(["gamma", "alpha", "beta"]);
  });

  it("returns null for a no-op drop on the same position", () => {
    const state = createDocState(["alpha", "beta"]);
    expect(reorderBlockTransaction(state, 0, 0)).toBeNull();
  });

  it("returns null when fromPos has no node", () => {
    const state = createDocState(["alpha"]);
    expect(reorderBlockTransaction(state, 999, 0)).toBeNull();
  });
});
