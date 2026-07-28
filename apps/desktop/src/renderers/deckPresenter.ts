import MarkdownIt from "markdown-it";

import type { DeckTransition } from "../lib/deckRun";

/**
 * CommonMark notes renderer with `html: false` so authored raw tags are escaped
 * as text. That is the same safety posture as the page markdown tokenizer —
 * no second HTML pass is required for speaker notes.
 */
const notesMarkdown = new MarkdownIt("commonmark", { html: false }).enable([
  "table",
  "strikethrough",
]);

/** Map a manifest transition onto a `data-transition` CSS hook. */
export function deckTransitionAttr(
  transition: DeckTransition | null | undefined,
  reducedMotion: boolean,
): string {
  if (reducedMotion) return "cut";
  const resolved = transition ?? { type: "cut" as const };
  if (resolved.type === "push") {
    return `push-${resolved.direction ?? "left"}`;
  }
  return resolved.type;
}

/** Human label for overview / notes chrome; keeps the raw id available separately. */
export function humanizeSlideId(id: string): string {
  const trimmed = id.trim();
  if (!trimmed) return id;
  return trimmed
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

/**
 * Render speaker notes as safe HTML for the presentation host (outside the
 * sandboxed slide iframe). Empty notes yield an empty string.
 */
export function renderDeckNotesHtml(notes?: string | null): string {
  const source = (notes ?? "").trim();
  if (!source) return "";
  return notesMarkdown.render(source);
}
