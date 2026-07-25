import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import type { JSONContent } from "@tiptap/core";

import {
  blockRangesForDocument,
  resolveBlockRange,
  structuralBlockId,
} from "./blockIds";

function editorRanges(content: JSONContent) {
  const editor = new Editor({
    extensions: [StarterKit],
    content,
  });
  const ranges = blockRangesForDocument(editor.state.doc);
  editor.destroy();
  return ranges;
}

const introPage: JSONContent = {
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 1 },
      content: [{ type: "text", text: "Intro" }],
    },
    {
      type: "paragraph",
      content: [{ type: "text", text: "First" }],
    },
    {
      type: "paragraph",
      content: [{ type: "text", text: "Second" }],
    },
  ],
};

describe("structural block ids", () => {
  it("assigns stable ids aligned with the indexer shape", () => {
    const counts = new Map<string, number>();
    expect(structuralBlockId([], "paragraph", counts)).toBe("root|paragraph#0");
    expect(structuralBlockId(["Intro"], "heading", counts)).toBe("Intro|heading#0");
    expect(structuralBlockId(["Intro"], "paragraph", counts)).toBe("Intro|paragraph#0");
  });

  it("maps headings and paragraphs in a simple page", () => {
    const ranges = editorRanges(introPage);
    expect(ranges.map((range) => range.blockId)).toEqual([
      "Intro|heading#0",
      "Intro|paragraph#0",
      "Intro|paragraph#1",
    ]);
  });

  it("resolves exact block ids and heading-path fallbacks", () => {
    const ranges = editorRanges({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Intro" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "First" }],
        },
      ],
    });
    expect(resolveBlockRange(ranges, "Intro|paragraph#0")).toEqual(ranges[1]);
    expect(resolveBlockRange(ranges, "Intro")).toEqual(ranges[0]);
    expect(resolveBlockRange(ranges, "Missing|paragraph#0")).toBeUndefined();
  });
});
