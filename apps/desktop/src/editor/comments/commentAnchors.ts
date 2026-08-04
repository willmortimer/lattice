import type { Editor } from "@tiptap/core";
import type { EditorState } from "@tiptap/pm/state";
import {
  absolutePositionToRelativePosition,
  relativePositionToAbsolutePosition,
  ySyncPluginKey,
} from "@tiptap/y-tiptap";
import * as Y from "yjs";

import { decodeRelativePosition, encodeRelativePosition } from "./commentCodec";

export const COLLAB_XML_FRAGMENT_FIELD = "default";

export interface CommentAnchorRange {
  anchorStart: string;
  anchorEnd: string;
  quote: string;
}

/** Create sticky anchors from a collaborative editor selection. */
export function createAnchorsFromSelection(editor: Editor): CommentAnchorRange | null {
  const { from, to, empty } = editor.state.selection;
  if (empty || from === to) return null;
  const start = encodePmPosition(editor.state, Math.min(from, to));
  const end = encodePmPosition(editor.state, Math.max(from, to));
  if (!start || !end) return null;
  return {
    anchorStart: start,
    anchorEnd: end,
    quote: editor.state.doc.textBetween(Math.min(from, to), Math.max(from, to), " "),
  };
}

/** Resolve a stored relative position to a ProseMirror absolute position. */
export function resolveAnchorToPmPosition(
  state: EditorState,
  encoded: string,
): number | null {
  const ystate = ySyncPluginKey.getState(state);
  if (!ystate?.doc || !ystate.type || !ystate.binding) return null;
  try {
    const relPos = decodeRelativePosition(encoded);
    const absolute = relativePositionToAbsolutePosition(
      ystate.doc,
      ystate.type,
      relPos,
      ystate.binding.mapping,
    );
    return absolute ?? null;
  } catch {
    return null;
  }
}

/**
 * Create a relative-position anchor against a shared type index.
 * Used by unit tests and any non-ProseMirror callers.
 */
export function createAnchorFromTypeIndex(
  type: Y.AbstractType<unknown>,
  index: number,
  assoc = 0,
): string {
  return encodeRelativePosition(Y.createRelativePositionFromTypeIndex(type, index, assoc));
}

/** Resolve an encoded relative position against a Y.Doc. */
export function resolveAnchorAbsoluteIndex(
  ydoc: Y.Doc,
  encoded: string,
): number | null {
  try {
    const abs = Y.createAbsolutePositionFromRelativePosition(
      decodeRelativePosition(encoded),
      ydoc,
    );
    return abs?.index ?? null;
  } catch {
    return null;
  }
}

function encodePmPosition(state: EditorState, absolutePos: number): string | null {
  const ystate = ySyncPluginKey.getState(state);
  if (!ystate?.type || !ystate.binding) return null;
  try {
    const relPos = absolutePositionToRelativePosition(
      absolutePos,
      ystate.type,
      ystate.binding.mapping,
    );
    return encodeRelativePosition(relPos);
  } catch {
    return null;
  }
}
