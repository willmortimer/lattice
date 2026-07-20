import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { invoke } from "./ipc";

export type SemanticStatusState =
  | "stopped"
  | "downloading"
  | "preparing"
  | "indexing"
  | "ready"
  | "degraded"
  | "failed";

export type SemanticStatus = {
  state: SemanticStatusState | string;
  pendingChunks: number | null;
  message: string | null;
  progressPercent?: number | null;
  providerId?: string | null;
  modelId?: string | null;
  dimensions?: number | null;
};

export type SemanticUiEvent = {
  type: "status";
  state: string;
  pendingChunks: number | null;
  message: string | null;
  progressPercent?: number | null;
  providerId?: string | null;
  modelId?: string | null;
  dimensions?: number | null;
};

/** Confirm-dialog copy for first-time model download (~640 MB Apache-2.0 GGUF). */
export const SEMANTIC_MODEL_CONFIRM =
  "Semantic search downloads Qwen3-Embedding-0.6B (Q8, ~640 MB, Apache-2.0). The model stays on this Mac and is never uploaded. Continue?";

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
    case "downloading":
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
  progressPercent?: number | null,
): string {
  if (!isSemanticStatusState(state)) {
    return `Unknown (${state})`;
  }
  switch (state) {
    case "stopped":
      return "Not prepared";
    case "downloading":
      return progressPercent != null ? `Downloading ${progressPercent}%` : "Downloading…";
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

/** Compact provider line for Settings (e.g. "llama.cpp · Qwen3-Embedding-0.6B · 512-d"). */
export function semanticProviderLabel(status: {
  providerId?: string | null;
  modelId?: string | null;
  dimensions?: number | null;
}): string | null {
  const parts: string[] = [];
  if (status.providerId) parts.push(status.providerId);
  if (status.modelId) parts.push(status.modelId);
  if (status.dimensions != null) parts.push(`${status.dimensions}-d`);
  return parts.length > 0 ? parts.join(" · ") : null;
}
