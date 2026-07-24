import type { UiMessageChunk } from "@lattice/agent-protocol";

export type FakeStreamOptions = {
  /** Text content to echo (defaults to prompt). */
  text?: string;
  /** Milliseconds between chunks (0 for sync tests). */
  chunkDelayMs?: number;
  signal?: AbortSignal;
};

/**
 * Deterministic fake provider: echoes the prompt as a few text-delta chunks.
 * Hermetic — no network, no Agents SDK.
 */
export async function* streamFakeChunks(
  prompt: string,
  options: FakeStreamOptions = {},
): AsyncGenerator<UiMessageChunk> {
  const text = options.text ?? `Echo: ${prompt}`;
  const delayMs = options.chunkDelayMs ?? 0;
  const signal = options.signal;
  const messageId = "fake-msg";

  const parts = splitIntoChunks(text, 3);

  yield { type: "text-start", id: messageId };

  for (const part of parts) {
    throwIfAborted(signal);
    if (delayMs > 0) {
      await sleep(delayMs, signal);
    }
    throwIfAborted(signal);
    yield { type: "text-delta", id: messageId, delta: part };
  }

  throwIfAborted(signal);
  yield { type: "text-end", id: messageId };
}

function splitIntoChunks(text: string, count: number): string[] {
  if (text.length === 0) {
    return [""];
  }
  const n = Math.max(1, Math.min(count, text.length));
  const size = Math.ceil(text.length / n);
  const chunks: string[] = [];
  for (let i = 0; i < text.length; i += size) {
    chunks.push(text.slice(i, i + size));
  }
  return chunks;
}

function throwIfAborted(signal: AbortSignal | undefined): void {
  if (signal?.aborted) {
    const err = new Error("Run cancelled");
    err.name = "AbortError";
    throw err;
  }
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      const err = new Error("Run cancelled");
      err.name = "AbortError";
      reject(err);
      return;
    }
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    const onAbort = () => {
      clearTimeout(timer);
      const err = new Error("Run cancelled");
      err.name = "AbortError";
      reject(err);
    };
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}
