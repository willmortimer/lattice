const CARET_COLORS = [
  "#e06c75",
  "#d19a66",
  "#98c379",
  "#56b6c2",
  "#61afef",
  "#c678dd",
  "#be5046",
  "#7f848e",
] as const;

export interface CollabCaretUser {
  name: string;
  color: string;
}

/** Stable local caret label + color for a Yjs awareness client id. */
export function collabCaretUser(clientId: number): CollabCaretUser {
  const color = CARET_COLORS[clientId % CARET_COLORS.length];
  return {
    name: `Editor ${clientId % 1000}`,
    color,
  };
}
