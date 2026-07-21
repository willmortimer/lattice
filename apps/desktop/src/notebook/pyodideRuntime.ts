import type {
  PyodideMountFile,
  PyodideRunPayload,
  PyodideWorkerRequest,
  PyodideWorkerResponse,
} from "./pyodideProtocol";
import { MAX_NOTEBOOK_OUTPUT_CHARS } from "./pyodideConfig";

export type { PyodideMountFile, PyodideRunPayload };

export class PyodideLoadError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PyodideLoadError";
  }
}

export class PyodideCancelledError extends Error {
  constructor() {
    super("Notebook run cancelled");
    this.name = "PyodideCancelledError";
  }
}

type Pending = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  kind: "ensure" | "run";
};

let worker: Worker | null = null;
let nextId = 1;
const pending = new Map<number, Pending>();

function rejectAll(error: Error): void {
  for (const entry of pending.values()) {
    entry.reject(error);
  }
  pending.clear();
}

function handleMessage(event: MessageEvent<PyodideWorkerResponse>): void {
  const response = event.data;
  const entry = pending.get(response.id);
  if (!entry) return;
  pending.delete(response.id);

  switch (response.type) {
    case "ready":
      entry.resolve(undefined);
      return;
    case "result":
      entry.resolve(response.payload);
      return;
    case "load-error":
      entry.reject(new PyodideLoadError(response.message));
      return;
    case "error":
      entry.reject(new Error(response.message));
      return;
    default: {
      const unreachable: never = response;
      entry.reject(new Error(`Unexpected worker response: ${JSON.stringify(unreachable)}`));
    }
  }
}

function ensureWorker(): Worker {
  if (worker) return worker;
  worker = new Worker(new URL("./pyodide.worker.ts", import.meta.url), { type: "module" });
  worker.onmessage = handleMessage;
  worker.onerror = (event) => {
    const error = new PyodideLoadError(event.message || "Pyodide worker failed");
    rejectAll(error);
    disposeWorker();
  };
  return worker;
}

function disposeWorker(): void {
  if (!worker) return;
  worker.terminate();
  worker = null;
}

export type RunPythonCellOptions = {
  signal?: AbortSignal;
  maxOutputChars?: number;
  /** Files to write into the Pyodide FS before executing the cell. */
  mountFiles?: PyodideMountFile[];
  /** Pyodide built-in packages to `loadPackage` before the cell (cached). */
  packages?: string[];
};

type WorkerRequestBody =
  | { type: "ensure" }
  | {
      type: "run";
      code: string;
      maxOutputChars?: number;
      mountFiles?: PyodideMountFile[];
      packages?: string[];
    };

function request<T>(message: WorkerRequestBody, signal?: AbortSignal): Promise<T> {
  if (signal?.aborted) return Promise.reject(new PyodideCancelledError());
  if (typeof Worker === "undefined") {
    return Promise.reject(new PyodideLoadError("Web Workers are unavailable in this environment."));
  }

  const id = nextId;
  nextId += 1;
  const runtime = ensureWorker();
  const envelope: PyodideWorkerRequest = { ...message, id };

  return new Promise<T>((resolve, reject) => {
    const finishReject = (error: Error) => {
      pending.delete(id);
      signal?.removeEventListener("abort", onAbort);
      reject(error);
    };
    const onAbort = () => {
      // Terminate drops in-flight Python; the next Run recreates the worker.
      rejectAll(new PyodideCancelledError());
      disposeWorker();
    };

    pending.set(id, {
      kind: message.type,
      resolve: (value) => {
        signal?.removeEventListener("abort", onAbort);
        resolve(value as T);
      },
      reject: finishReject,
    });
    signal?.addEventListener("abort", onAbort, { once: true });
    if (signal?.aborted) {
      onAbort();
      return;
    }
    // Structured clone carries Uint8Array file mounts; cancel still terminates the worker.
    runtime.postMessage(envelope);
  });
}

/** Warm the Pyodide worker (CDN load) without executing a cell. */
export function ensurePyodide(signal?: AbortSignal): Promise<void> {
  return request<void>({ type: "ensure" }, signal);
}

/** Execute one Python cell; cancel via AbortSignal by terminating the worker. */
export function runPythonCell(
  code: string,
  signalOrOptions?: AbortSignal | RunPythonCellOptions,
  maxOutputChars = MAX_NOTEBOOK_OUTPUT_CHARS,
): Promise<PyodideRunPayload> {
  const options: RunPythonCellOptions =
    signalOrOptions instanceof AbortSignal || signalOrOptions === undefined
      ? { signal: signalOrOptions, maxOutputChars }
      : { maxOutputChars, ...signalOrOptions };
  return request<PyodideRunPayload>(
    {
      type: "run",
      code,
      maxOutputChars: options.maxOutputChars ?? MAX_NOTEBOOK_OUTPUT_CHARS,
      mountFiles: options.mountFiles,
      packages: options.packages,
    },
    options.signal,
  );
}

/** Drop a warm worker (tests / teardown). */
export function resetPyodideRuntime(): void {
  rejectAll(new PyodideCancelledError());
  disposeWorker();
}
