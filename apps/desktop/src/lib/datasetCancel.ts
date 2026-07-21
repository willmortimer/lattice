import { invoke } from "@tauri-apps/api/core";

/** Thrown when an AbortSignal cancels a dataset Tauri invoke. */
export class DatasetRequestAbortedError extends Error {
  constructor() {
    super("Dataset request was cancelled");
    this.name = "AbortError";
  }
}

export function isDatasetRequestAborted(error: unknown): boolean {
  if (error instanceof DatasetRequestAbortedError) return true;
  if (error instanceof DOMException && error.name === "AbortError") return true;
  if (error instanceof Error && error.name === "AbortError") return true;
  return false;
}

export function newDatasetSessionId(): string {
  return crypto.randomUUID();
}

/** Flip the backend cancel token / interrupt DuckDB for an in-flight session. */
export function cancelDatasetQuery(sessionId: string): Promise<boolean> {
  return invoke<boolean>("cancel_dataset_query", { sessionId });
}

function assertActive(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DatasetRequestAbortedError();
}

/**
 * Race a Tauri invoke against AbortSignal, mirroring `resourceRuntime` `guarded`.
 * When `sessionId` is set, abort also invokes `cancel_dataset_query`.
 */
export async function guardedDatasetRequest<T>(
  makeRequest: () => Promise<T>,
  signal?: AbortSignal,
  sessionId?: string,
): Promise<T> {
  assertActive(signal);
  const request = makeRequest();
  if (!signal) return request;

  let onAbort: (() => void) | undefined;
  const aborted = new Promise<never>((_, reject) => {
    onAbort = () => {
      if (sessionId) {
        void cancelDatasetQuery(sessionId).catch(() => undefined);
      }
      reject(new DatasetRequestAbortedError());
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });

  try {
    const result = await Promise.race([request, aborted]);
    assertActive(signal);
    return result;
  } finally {
    if (onAbort) signal.removeEventListener("abort", onAbort);
  }
}
