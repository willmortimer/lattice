export type SaveState =
  | { status: "idle" }
  | { status: "dirty" }
  | { status: "saving" }
  | { status: "saved" }
  | { status: "conflict"; message: string }
  | { status: "error"; message: string };

/** Whether `state` represents an edit not yet durably saved. */
export function isUnsaved(state: SaveState): boolean {
  return state.status !== "idle" && state.status !== "saved";
}

/** Short label for save-state indicators in shell chrome. */
export function saveIndicatorText(state: SaveState): string {
  switch (state.status) {
    case "idle":
      return "";
    case "dirty":
      return "Edited";
    case "saving":
      return "Saving…";
    case "saved":
      return "Saved";
    case "conflict":
      return "Save conflict";
    case "error":
      return "Save failed";
    default:
      return state satisfies never;
  }
}
