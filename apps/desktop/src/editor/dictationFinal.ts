import type { EditorState, Transaction } from "@tiptap/pm/state";

import {
  dictationProvisionalKey,
  type DictationProvisionalState,
} from "./DictationProvisional";

export type DictationSegment =
  | { kind: "text"; value: string }
  | { kind: "newline" }
  | { kind: "paragraph" };

const VOICE_MARKER_RE = /\b(new line|new paragraph)\b/gi;

/** Split a final transcript into plain text and basic voice structure markers. */
export function parseDictationSegments(raw: string): DictationSegment[] {
  const trimmed = raw.trim();
  if (!trimmed) return [];

  const segments: DictationSegment[] = [];
  let lastIndex = 0;
  for (const match of trimmed.matchAll(VOICE_MARKER_RE)) {
    const start = match.index ?? 0;
    if (start > lastIndex) {
      const chunk = trimmed.slice(lastIndex, start).replace(/\s+/g, " ").trim();
      if (chunk) segments.push({ kind: "text", value: chunk });
    }
    const marker = match[1]?.toLowerCase();
    segments.push(marker === "new paragraph" ? { kind: "paragraph" } : { kind: "newline" });
    lastIndex = start + match[0].length;
  }
  const tail = trimmed.slice(lastIndex).replace(/\s+/g, " ").trim();
  if (tail) segments.push({ kind: "text", value: tail });
  return segments;
}

function clampInsertPos(doc: EditorState["doc"], from: number): number {
  return Math.min(Math.max(1, from), doc.content.size);
}

function appendSegments(tr: Transaction, segments: DictationSegment[], from: number): Transaction {
  let pos = clampInsertPos(tr.doc, from);
  const hardBreak = tr.doc.type.schema.nodes.hardBreak;

  for (const segment of segments) {
    if (segment.kind === "text") {
      tr = tr.insertText(segment.value, pos);
      pos += segment.value.length;
      continue;
    }
    if (segment.kind === "newline") {
      if (hardBreak) {
        tr = tr.insert(pos, hardBreak.create());
        pos += 1;
      } else {
        tr = tr.insertText("\n", pos);
        pos += 1;
      }
      continue;
    }
    const $pos = tr.doc.resolve(pos);
    if ($pos.parent.isTextblock && $pos.parentOffset > 0) {
      tr = tr.split(pos, 1);
      pos = tr.mapping.map(pos);
      continue;
    }
    if (hardBreak) {
      tr = tr.insert(pos, hardBreak.create());
      pos += 1;
    }
  }
  return tr;
}

/**
 * Clear provisional decoration and insert the final transcript in one
 * ProseMirror transaction so undo reverts the whole utterance.
 */
export function commitDictationFinalTransaction(
  state: EditorState,
  text: string,
  from: number,
): Transaction {
  const clear: DictationProvisionalState = { text: "", from: 0 };
  let tr = state.tr.setMeta(dictationProvisionalKey, clear);

  const segments = parseDictationSegments(text);
  if (segments.length === 0) return tr;

  return appendSegments(tr, segments, from);
}
