import type {
  StructuredParseLimits,
  StructuredParseResult,
  StructuredSyntax,
} from "./structuredParserCore";

export type { StructuredNode, StructuredParseResult, StructuredSyntax } from "./structuredParserCore";

let requestId = 0;

export function parseStructuredInWorker(
  source: string,
  syntax: StructuredSyntax,
  signal?: AbortSignal,
  limits?: Partial<StructuredParseLimits>,
): Promise<StructuredParseResult> {
  if (signal?.aborted) return Promise.reject(new DOMException("Parse cancelled", "AbortError"));
  if (typeof Worker === "undefined") {
    return import("./structuredParserCore").then(({ parseStructuredSource }) => {
      if (signal?.aborted) throw new DOMException("Parse cancelled", "AbortError");
      return parseStructuredSource(source, syntax, limits);
    });
  }

  const worker = new Worker(new URL("./structuredParser.worker.ts", import.meta.url), { type: "module" });
  const id = ++requestId;
  return new Promise<StructuredParseResult>((resolve, reject) => {
    let settled = false;
    const finish = (callback: () => void) => {
      if (settled) return;
      settled = true;
      signal?.removeEventListener("abort", abort);
      worker.terminate();
      callback();
    };
    const abort = () => finish(() => reject(new DOMException("Parse cancelled", "AbortError")));
    worker.onmessage = (event: MessageEvent<{ id: number; result: StructuredParseResult }>) => {
      if (event.data.id !== id) return;
      finish(() => resolve(event.data.result));
    };
    worker.onerror = (event) => finish(() => reject(event.error ?? new Error(event.message)));
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
    else worker.postMessage({ id, source, syntax, limits });
  });
}
