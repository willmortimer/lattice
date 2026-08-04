import * as Y from "yjs";

import type { CommentAnchorRange } from "./commentAnchors";

/** Root Y.Map key for page sticky comments (lives alongside the XmlFragment). */
export const COMMENTS_MAP_KEY = "comments";

/**
 * Sticky comment schema (each entry is a nested `Y.Map` under `comments`):
 *
 * | key           | type    | notes                                      |
 * |---------------|---------|--------------------------------------------|
 * | id            | string  | stable comment id (also the outer map key) |
 * | body          | string  | thread body                                |
 * | author        | string  | local display label                        |
 * | createdAt     | number  | unix ms                                    |
 * | resolved      | boolean | resolve/unresolve                          |
 * | quote         | string  | selection snapshot at create time          |
 * | anchorStart   | string  | JSON Y.RelativePosition                    |
 * | anchorEnd     | string  | JSON Y.RelativePosition                    |
 */
export interface StickyComment {
  id: string;
  body: string;
  author: string;
  createdAt: number;
  resolved: boolean;
  quote: string;
  anchorStart: string;
  anchorEnd: string;
}

export interface CreateStickyCommentInput {
  body: string;
  author: string;
  anchors: CommentAnchorRange;
  id?: string;
  createdAt?: number;
}

export function getCommentsMap(ydoc: Y.Doc): Y.Map<unknown> {
  return ydoc.getMap(COMMENTS_MAP_KEY);
}

export function listStickyComments(ydoc: Y.Doc): StickyComment[] {
  const comments = getCommentsMap(ydoc);
  const out: StickyComment[] = [];
  comments.forEach((value, key) => {
    const parsed = readCommentEntry(key, value);
    if (parsed) out.push(parsed);
  });
  out.sort((a, b) => a.createdAt - b.createdAt);
  return out;
}

export function createStickyComment(ydoc: Y.Doc, input: CreateStickyCommentInput): StickyComment {
  const id = input.id ?? crypto.randomUUID();
  const createdAt = input.createdAt ?? Date.now();
  const entry = new Y.Map<unknown>();
  entry.set("id", id);
  entry.set("body", input.body);
  entry.set("author", input.author);
  entry.set("createdAt", createdAt);
  entry.set("resolved", false);
  entry.set("quote", input.anchors.quote);
  entry.set("anchorStart", input.anchors.anchorStart);
  entry.set("anchorEnd", input.anchors.anchorEnd);

  getCommentsMap(ydoc).set(id, entry);

  return {
    id,
    body: input.body,
    author: input.author,
    createdAt,
    resolved: false,
    quote: input.anchors.quote,
    anchorStart: input.anchors.anchorStart,
    anchorEnd: input.anchors.anchorEnd,
  };
}

export function setStickyCommentResolved(
  ydoc: Y.Doc,
  commentId: string,
  resolved: boolean,
): boolean {
  const entry = getCommentsMap(ydoc).get(commentId);
  if (!(entry instanceof Y.Map)) return false;
  entry.set("resolved", resolved);
  return true;
}

function readCommentEntry(key: string, value: unknown): StickyComment | null {
  if (!(value instanceof Y.Map)) return null;
  const id = asString(value.get("id")) ?? key;
  const body = asString(value.get("body"));
  const author = asString(value.get("author"));
  const createdAt = asNumber(value.get("createdAt"));
  const resolved = asBoolean(value.get("resolved"));
  const quote = asString(value.get("quote"));
  const anchorStart = asString(value.get("anchorStart"));
  const anchorEnd = asString(value.get("anchorEnd"));
  if (
    body === null ||
    author === null ||
    createdAt === null ||
    resolved === null ||
    quote === null ||
    anchorStart === null ||
    anchorEnd === null
  ) {
    return null;
  }
  return { id, body, author, createdAt, resolved, quote, anchorStart, anchorEnd };
}

function asString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asBoolean(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}
