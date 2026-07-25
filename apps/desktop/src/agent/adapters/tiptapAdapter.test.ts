import { describe, expect, it, vi } from "vitest";
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import type { JSONContent } from "@tiptap/core";

import { resolveBlockRange, blockRangesForDocument } from "./blockIds";
import { createMarkdownBlockAdapter } from "./tiptapAdapter";

const introParagraphPage: JSONContent = {
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 1 },
      content: [{ type: "text", text: "Intro" }],
    },
    {
      type: "paragraph",
      content: [{ type: "text", text: "Target paragraph" }],
    },
  ],
};

function createDocEditor(content: JSONContent) {
  return new Editor({
    extensions: [StarterKit],
    content,
  });
}

describe("markdown block adapter", () => {
  it("dispatches highlight commands for a resolved block id", () => {
    const editor = createDocEditor(introParagraphPage);
    const expectedRange = resolveBlockRange(
      blockRangesForDocument(editor.state.doc),
      "Intro|paragraph#0",
    );
    expect(expectedRange).toBeDefined();

    const highlightAgentAnchor = vi.fn(() => true);
    const clearAgentAnchorOverlay = vi.fn(() => true);
    const boundEditor = {
      state: editor.state,
      view: editor.view,
      commands: {
        highlightAgentAnchor,
        clearAgentAnchorOverlay,
      },
    } as unknown as Editor;

    const adapter = createMarkdownBlockAdapter(boundEditor, "Notes/Page.md");
    const clear = adapter.highlight(
      {
        kind: "markdown-block",
        resourceId: "Notes/Page.md",
        blockId: "Intro|paragraph#0",
      },
      { overlayId: "overlay-1", purpose: "attention" },
    );

    expect(highlightAgentAnchor).toHaveBeenCalledWith(
      "overlay-1",
      expectedRange!.from,
      expectedRange!.to,
      "attention",
    );
    clear();
    expect(clearAgentAnchorOverlay).toHaveBeenCalledWith("overlay-1");
    editor.destroy();
  });

  it("ignores anchors for a different resource id", () => {
    const editor = createDocEditor({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Hello" }],
        },
      ],
    });
    const highlightAgentAnchor = vi.fn(() => true);
    const boundEditor = {
      state: editor.state,
      view: editor.view,
      commands: { highlightAgentAnchor, clearAgentAnchorOverlay: vi.fn() },
    } as unknown as Editor;

    const adapter = createMarkdownBlockAdapter(boundEditor, "Notes/Page.md");
    adapter.highlight(
      {
        kind: "markdown-block",
        resourceId: "Other.md",
        blockId: "root|paragraph#0",
      },
      { overlayId: "overlay-2", purpose: "evidence" },
    );
    expect(highlightAgentAnchor).not.toHaveBeenCalled();
    editor.destroy();
  });
});
