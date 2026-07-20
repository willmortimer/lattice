export const BROWSER_UNDO_UNAVAILABLE_MESSAGE =
  "Undo is not available in the browser demo.";

/** Workspace undo calls `undo_last` over IPC; the browser fixture has no command history. */
export function browserUndoBlocked(inBrowser: boolean): boolean {
  return inBrowser;
}
