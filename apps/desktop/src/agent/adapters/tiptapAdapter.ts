import type { Editor } from "@tiptap/core";
import { TextSelection } from "@tiptap/pm/state";

import type { MarkdownBlockAnchor } from "@lattice/agent-protocol";

import { blockRangesForDocument, resolveBlockRange } from "./blockIds";
import type {
  AnchorHighlightPurpose,
  AnchorRevealBehavior,
  MarkdownBlockAnchorAdapter,
} from "./types";

function blockRangeForAnchor(editor: Editor, blockId: string) {
  const ranges = blockRangesForDocument(editor.state.doc);
  return resolveBlockRange(ranges, blockId);
}

export function createMarkdownBlockAdapter(
  editor: Editor,
  resourceId: string,
): MarkdownBlockAnchorAdapter {
  return {
    kind: "markdown-block",
    resourceId,

    async reveal(anchor: MarkdownBlockAnchor, behavior: AnchorRevealBehavior): Promise<void> {
      if (anchor.resourceId !== resourceId) return;
      const range = blockRangeForAnchor(editor, anchor.blockId);
      if (!range) return;
      if (behavior === "peek") return;
      const { doc } = editor.state;
      const selection = TextSelection.create(doc, range.from, Math.min(range.to, doc.content.size));
      const tr = editor.state.tr.setSelection(selection).scrollIntoView();
      editor.view.dispatch(tr);
    },

    highlight(
      anchor: MarkdownBlockAnchor,
      options: { overlayId: string; purpose: AnchorHighlightPurpose },
    ): () => void {
      if (anchor.resourceId !== resourceId) {
        return () => undefined;
      }
      const range = blockRangeForAnchor(editor, anchor.blockId);
      if (!range) {
        return () => undefined;
      }
      editor.commands.highlightAgentAnchor(
        options.overlayId,
        range.from,
        range.to,
        options.purpose,
      );
      return () => {
        editor.commands.clearAgentAnchorOverlay(options.overlayId);
      };
    },

    getScreenRect(anchor: MarkdownBlockAnchor): DOMRect | null {
      if (anchor.resourceId !== resourceId) return null;
      const range = blockRangeForAnchor(editor, anchor.blockId);
      if (!range) return null;
      try {
        const start = editor.view.coordsAtPos(range.from);
        const end = editor.view.coordsAtPos(Math.min(range.to, editor.state.doc.content.size));
        return new DOMRect(
          start.left,
          start.top,
          Math.max(0, end.right - start.left),
          Math.max(0, end.bottom - start.top),
        );
      } catch {
        return null;
      }
    },
  };
}
