import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { invoke } from "./ipc";

export type SemanticStatusState =
  | "stopped"
  | "preparing"
  | "indexing"
  | "ready"
  | "degraded"
  | "failed";

export type SemanticStatus = {
  state: SemanticStatusState | string;
  pendingChunks: number | null;
  message: string | null;
};

export type SemanticUiEvent = {
  type: "status";
  state: string;
  pendingChunks: number | null;
  message: string | null;
};

export async function getSemanticStatus(root: string): Promise<SemanticStatus> {
  return invoke<SemanticStatus>("semantic_status", { root });
}

export async function enableSemanticSearch(root: string): Promise<SemanticStatus> {
  return invoke<SemanticStatus>("semantic_enable", { root });
}

export async function disableSemanticSearch(root: string): Promise<SemanticStatus> {
  return invoke<SemanticStatus>("semantic_disable", { root });
}

export async function listenSemanticEvents(
  onEvent: (event: SemanticUiEvent) => void,
): Promise<UnlistenFn> {
  return listen<SemanticUiEvent>("semantic-event", (event) => {
    onEvent(event.payload);
  });
}

export function isSemanticStatusState(value: string): value is SemanticStatusState {
  switch (value) {
    case "stopped":
    case "preparing":
    case "indexing":
    case "ready":
    case "degraded":
    case "failed":
      return true;
    default:
      return false;
  }
}

/** Settings / status row label for a semantic lifecycle state. */
export function semanticStatusLabel(
  state: SemanticStatusState | string,
  pendingChunks: number | null | undefined,
): string {
  if (!isSemanticStatusState(state)) {
    return `Unknown (${state})`;
  }
  switch (state) {
    case "stopped":
      return "Not prepared";
    case "preparing":
      return "Preparing…";
    case "indexing":
      return pendingChunks != null && pendingChunks > 0
        ? `Indexing (${pendingChunks} pending)`
        : "Indexing…";
    case "ready":
      return "Ready";
    case "degraded":
      return "Degraded (keyword search still works)";
    case "failed":
      return "Failed";
    default: {
      const _exhaustive: never = state;
      return _exhaustive;
    }
  }
}
