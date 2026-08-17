/** Trim rename prompt input; null when cancelled or empty. */
export function normalizeRenameInput(input: string | null | undefined): string | null {
  if (input == null) {
    return null;
  }
  const trimmed = input.trim();
  return trimmed || null;
}

export const DELETE_THREAD_CONFIRM_MESSAGE =
  "Delete this thread? This permanently removes the thread and its messages.";

/** Gate delete behind a confirm dialog. */
export function shouldProceedWithDelete(
  confirm: (message: string) => boolean = window.confirm.bind(window),
): boolean {
  return confirm(DELETE_THREAD_CONFIRM_MESSAGE);
}

export type SelectionAfterRemoval =
  | { kind: "unchanged" }
  | { kind: "select"; threadId: string }
  | { kind: "new" };

/** Pick the next thread selection after archive/delete removes the active thread. */
export function selectionAfterThreadRemoval(
  removedThreadId: string,
  selectedThreadId: string,
  remainingThreadIds: readonly string[],
): SelectionAfterRemoval {
  if (!removedThreadId || removedThreadId !== selectedThreadId) {
    return { kind: "unchanged" };
  }
  const nextId = remainingThreadIds.find((id) => id !== removedThreadId);
  if (nextId) {
    return { kind: "select", threadId: nextId };
  }
  return { kind: "new" };
}
