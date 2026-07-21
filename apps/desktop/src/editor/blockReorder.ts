import type { Node as PmNode } from "@tiptap/pm/model";
import type { EditorState, Transaction } from "@tiptap/pm/state";

/**
 * Resolve the drag source block position. Prefer MIME / plain payloads when
 * present; fall back to the module-level pos so WKWebView drops still work
 * when custom MIME types are missing from `dataTransfer` during dragover.
 */
export function resolveBlockDragFromPos(
  fromMime: string | undefined,
  fromPlain: string | undefined,
  activeFromPos: number | null,
): number | null {
  const raw = fromMime || fromPlain || (activeFromPos !== null ? String(activeFromPos) : "");
  if (!raw) return null;
  const fromPos = Number(raw);
  return Number.isFinite(fromPos) ? fromPos : null;
}

/** True when a block drag is armed via module-level pos (not MIME types alone). */
export function isBlockDragArmed(activeFromPos: number | null): boolean {
  return activeFromPos !== null;
}

/** Map a document position to the start of its top-level block under `doc`. */
export function topLevelBlockPos(doc: PmNode, pos: number): number {
  const $pos = doc.resolve(pos);
  for (let depth = $pos.depth; depth > 0; depth -= 1) {
    if ($pos.node(depth - 1).type.name === "doc") {
      return $pos.before(depth);
    }
  }
  return 0;
}

/**
 * Build a transaction that moves the top-level block at `fromPos` so it
 * begins at `toPos`. Returns null when the move is a no-op or invalid.
 */
export function reorderBlockTransaction(
  state: EditorState,
  fromPos: number,
  toPos: number,
): Transaction | null {
  if (fromPos < 0 || fromPos >= state.doc.content.size) return null;
  const node = state.doc.nodeAt(fromPos);
  if (!node) return null;
  if (toPos === fromPos || toPos === fromPos + node.nodeSize) return null;

  let tr = state.tr.delete(fromPos, fromPos + node.nodeSize);
  const mappedTo = tr.mapping.map(toPos, -1);
  tr = tr.insert(mappedTo, node);
  return tr.scrollIntoView();
}
