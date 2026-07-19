import type { Extensions } from "@tiptap/core";
import { ReactNodeViewRenderer } from "@tiptap/react";

import { BlockDragHandle } from "./BlockDragHandle";
import { CodeBlockView } from "./CodeBlockView";
import { DictationProvisional } from "./DictationProvisional";
import { editorExtensions } from "./extensions";
import { ImageView } from "./ImageView";
import { LatticeEmbedView } from "./LatticeEmbedView";

/**
 * `editorExtensions` with read-view React node views for `image`,
 * `codeBlock`, and `latticeEmbed`. `.extend()` only adds `addNodeView`,
 * so the schema — what a document can contain — stays identical to the
 * bare codec list in `extensions.ts`.
 */
export const richEditorExtensions: Extensions = editorExtensions.map((extension) => {
  if (extension.name === "starterKit") {
    return extension.extend({
      addExtensions() {
        return this.parent?.().map((child) => {
          if (child.name === "codeBlock") {
            return child.extend({ addNodeView: () => ReactNodeViewRenderer(CodeBlockView) });
          }
          return child;
        });
      },
    });
  }
  if (extension.name === "image") {
    return extension.extend({ addNodeView: () => ReactNodeViewRenderer(ImageView) });
  }
  if (extension.name === "latticeEmbed") {
    return extension.extend({ addNodeView: () => ReactNodeViewRenderer(LatticeEmbedView) });
  }
  return extension;
});

/** Live editor: rich node views plus edit-only chrome (drag handles, dictation). */
export const liveEditorExtensions: Extensions = [
  ...richEditorExtensions,
  BlockDragHandle,
  DictationProvisional,
];
