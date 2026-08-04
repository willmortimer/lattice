import type { Extensions } from "@tiptap/core";
import Collaboration from "@tiptap/extension-collaboration";
import { ReactNodeViewRenderer } from "@tiptap/react";
import type * as Y from "yjs";

import { AgentAnchorHighlight } from "../agent/adapters/AgentAnchorHighlight";
import { BlockDragHandle } from "./BlockDragHandle";
import { CodeBlockView } from "./CodeBlockView";
import { DictationProvisional } from "./DictationProvisional";
import { editorExtensions } from "./extensions";
import { ImageView } from "./ImageView";
import { LatticeEmbedView } from "./LatticeEmbedView";

function mapRichEditorExtensions(undoRedo: boolean): Extensions {
  return editorExtensions.map((extension) => {
    if (extension.name === "starterKit") {
      return extension
        .extend({
          addExtensions() {
            return this.parent?.().map((child) => {
              if (child.name === "codeBlock") {
                return child.extend({ addNodeView: () => ReactNodeViewRenderer(CodeBlockView) });
              }
              return child;
            });
          },
        })
        .configure({ undoRedo });
    }
    if (extension.name === "image") {
      return extension.extend({ addNodeView: () => ReactNodeViewRenderer(ImageView) });
    }
    if (extension.name === "latticeEmbed") {
      return extension.extend({ addNodeView: () => ReactNodeViewRenderer(LatticeEmbedView) });
    }
    return extension;
  });
}

/**
 * `editorExtensions` with read-view React node views for `image`,
 * `codeBlock`, and `latticeEmbed`. `.extend()` only adds `addNodeView`,
 * so the schema — what a document can contain — stays identical to the
 * bare codec list in `extensions.ts`.
 */
export const richEditorExtensions: Extensions = mapRichEditorExtensions(true);

/** Live editor: rich node views plus edit-only chrome (drag handles, dictation). */
export const liveEditorExtensions: Extensions = [
  ...richEditorExtensions,
  BlockDragHandle,
  DictationProvisional,
  AgentAnchorHighlight,
];

/** Collaborative live editor bound to a shared Y.Doc (undo handled by Yjs). */
export function collabLiveEditorExtensions(ydoc: Y.Doc): Extensions {
  return [
    ...mapRichEditorExtensions(false),
    Collaboration.configure({
      document: ydoc,
    }),
    BlockDragHandle,
    DictationProvisional,
    AgentAnchorHighlight,
  ];
}
