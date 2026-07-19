import { describe, expect, it } from "vitest";
import { flattenExtensions } from "@tiptap/core";

import { editorExtensions } from "./extensions";
import { liveEditorExtensions, richEditorExtensions } from "./richEditorExtensions";

function extensionByName(extensions: readonly { name: string }[], name: string) {
  return extensions.find((extension) => extension.name === name);
}

function flattenedByName(extensions: readonly { name: string }[]) {
  return flattenExtensions([...extensions]);
}

describe("richEditorExtensions", () => {
  it("keeps the same extension count as the bare codec schema", () => {
    expect(richEditorExtensions).toHaveLength(editorExtensions.length);
  });

  it("adds node views for rich-rendered block types", () => {
    const bare = flattenedByName(editorExtensions);
    const rich = flattenedByName(richEditorExtensions);

    for (const name of ["image", "codeBlock", "latticeEmbed"] as const) {
      const bareExtension = extensionByName(bare, name);
      const richExtension = extensionByName(rich, name);
      expect(bareExtension).toBeDefined();
      expect(richExtension).toBeDefined();
      expect(richExtension).not.toBe(bareExtension);
      expect(richExtension?.config.addNodeView).toBeTypeOf("function");
    }
  });

  it("is distinct from the bare codec extension list", () => {
    expect(richEditorExtensions).not.toBe(editorExtensions);
    expect(richEditorExtensions.some((extension, index) => extension !== editorExtensions[index])).toBe(
      true,
    );
  });

  it("extends live editing with drag handles and dictation chrome", () => {
    expect(liveEditorExtensions).toHaveLength(richEditorExtensions.length + 2);
    expect(extensionByName(liveEditorExtensions, "blockDragHandle")).toBeDefined();
    expect(extensionByName(liveEditorExtensions, "dictationProvisional")).toBeDefined();
  });
});
