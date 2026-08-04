import { useState } from "react";
import type { Editor } from "@tiptap/core";
import type * as Y from "yjs";
import { TextSelection } from "@tiptap/pm/state";

import { createAnchorsFromSelection, resolveAnchorToPmPosition } from "./commentAnchors";
import {
  createStickyComment,
  setStickyCommentResolved,
  type StickyComment,
} from "./commentStore";
import { useStickyComments } from "./useStickyComments";

export interface CommentsPanelProps {
  ydoc: Y.Doc;
  editor: Editor | null;
  author: string;
  open: boolean;
}

export function CommentsPanel({ ydoc, editor, author, open }: CommentsPanelProps) {
  const comments = useStickyComments(ydoc);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const openCount = comments.filter((comment) => !comment.resolved).length;

  function handleCreate() {
    if (!editor) {
      setError("Editor is not ready.");
      return;
    }
    const body = draft.trim();
    if (!body) {
      setError("Write a comment first.");
      return;
    }
    const anchors = createAnchorsFromSelection(editor);
    if (!anchors) {
      setError("Select text in the page to anchor a comment.");
      return;
    }
    createStickyComment(ydoc, { body, author, anchors });
    setDraft("");
    setError(null);
  }

  function handleJump(comment: StickyComment) {
    if (!editor) return;
    const from = resolveAnchorToPmPosition(editor.state, comment.anchorStart);
    const to = resolveAnchorToPmPosition(editor.state, comment.anchorEnd);
    if (from === null || to === null) {
      setError("Comment anchor could not be resolved (text may have been deleted).");
      return;
    }
    const lo = Math.min(from, to);
    const hi = Math.max(from, to);
    editor.view.dispatch(
      editor.state.tr.setSelection(TextSelection.create(editor.state.doc, lo, hi)).scrollIntoView(),
    );
    editor.view.focus();
    setError(null);
  }

  return (
    <aside className="page-comments-panel" aria-label="Page comments">
      <div className="page-comments-header">
        <strong>Comments</strong>
        <span className="page-comments-count">{openCount} open</span>
      </div>

      <div className="page-comments-compose">
        <textarea
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="Comment on the current selection…"
          rows={3}
          aria-label="New comment"
        />
        <button type="button" className="page-comments-action" onClick={handleCreate}>
          Add comment
        </button>
      </div>

      {error ? (
        <p className="page-comments-error" role="alert">
          {error}
        </p>
      ) : null}

      <ul className="page-comments-list">
        {comments.length === 0 ? (
          <li className="page-comments-empty">No comments yet. Select text and add one.</li>
        ) : (
          comments.map((comment) => (
            <li
              key={comment.id}
              className={
                comment.resolved
                  ? "page-comments-item page-comments-item-resolved"
                  : "page-comments-item"
              }
            >
              <button
                type="button"
                className="page-comments-quote"
                onClick={() => handleJump(comment)}
                title="Jump to anchor"
              >
                “{comment.quote || "…"}”
              </button>
              <p className="page-comments-body">{comment.body}</p>
              <div className="page-comments-meta">
                <span>{comment.author}</span>
                <button
                  type="button"
                  className="page-comments-action"
                  onClick={() => setStickyCommentResolved(ydoc, comment.id, !comment.resolved)}
                >
                  {comment.resolved ? "Unresolve" : "Resolve"}
                </button>
              </div>
            </li>
          ))
        )}
      </ul>
    </aside>
  );
}
