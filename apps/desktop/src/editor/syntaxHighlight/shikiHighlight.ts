import type { ShikiHighlightRequest, ShikiHighlightResponse } from "./shikiProtocol";

let requestId = 0;
let sharedWorker: Worker | null = null;

type Pending = {
  resolve: (html: string) => void;
  reject: (error: Error) => void;
};

const pending = new Map<number, Pending>();

function getSharedWorker(): Worker {
  if (!sharedWorker) {
    sharedWorker = new Worker(new URL("./shiki.worker.ts", import.meta.url), {
      type: "module",
    });
    sharedWorker.onmessage = (event: MessageEvent<ShikiHighlightResponse>) => {
      const response = event.data;
      const entry = pending.get(response.id);
      if (!entry) return;
      pending.delete(response.id);
      if (response.ok) entry.resolve(response.html);
      else entry.reject(new Error(response.error));
    };
    sharedWorker.onerror = (event) => {
      const error = event.error instanceof Error ? event.error : new Error(event.message);
      for (const [id, entry] of pending) {
        pending.delete(id);
        entry.reject(error);
      }
    };
  }
  return sharedWorker;
}

/**
 * Pull the inner `<code>` markup from Shiki's classic `codeToHtml` output so
 * we can overlay it without nesting another `<pre>`.
 */
export function extractShikiCodeInnerHtml(shikiHtml: string): string {
  const match = /<code[^>]*>([\s\S]*)<\/code>/i.exec(shikiHtml);
  return match?.[1] ?? shikiHtml;
}

/**
 * Off-thread (preferred) or main-thread syntax highlighting with abort support.
 * Mirrors the cancel pattern in `viewers/text/structuredParser.ts`, but keeps a
 * long-lived worker so `createHighlighter` runs once.
 *
 * The highlighter/grammars stay behind the worker (or a dynamic import) so the
 * page shell does not pay for TextMate payloads on first paint.
 */
export function highlightCodeInWorker(
  code: string,
  lang: string,
  signal?: AbortSignal,
): Promise<string> {
  if (signal?.aborted) {
    return Promise.reject(new DOMException("Highlight cancelled", "AbortError"));
  }

  if (typeof Worker === "undefined") {
    return import("./latticeHighlighter").then(({ highlightWithLatticeShiki }) => {
      if (signal?.aborted) throw new DOMException("Highlight cancelled", "AbortError");
      return highlightWithLatticeShiki(code, lang);
    });
  }

  const worker = getSharedWorker();
  const id = ++requestId;
  return new Promise<string>((resolve, reject) => {
    let settled = false;
    const finish = (callback: () => void) => {
      if (settled) return;
      settled = true;
      signal?.removeEventListener("abort", onAbort);
      pending.delete(id);
      callback();
    };
    const onAbort = () =>
      finish(() => reject(new DOMException("Highlight cancelled", "AbortError")));

    pending.set(id, {
      resolve: (html) => finish(() => resolve(html)),
      reject: (error) => finish(() => reject(error)),
    });

    signal?.addEventListener("abort", onAbort, { once: true });
    if (signal?.aborted) {
      onAbort();
      return;
    }

    const request: ShikiHighlightRequest = { id, code, lang };
    worker.postMessage(request);
  });
}
