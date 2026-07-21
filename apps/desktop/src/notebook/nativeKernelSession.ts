import { invoke } from "@tauri-apps/api/core";
import type { KernelExecuteOptions, KernelRunPayload, KernelSession } from "./kernelSession";

export type NativeKernelSessionOptions = {
  /** Workspace root passed to `kernel_start` (capability + cwd gate). */
  root: string;
};

/** Tauri `kernel_start` response (camelCase). */
type KernelStartResponse = {
  sessionId: string;
};

/**
 * One Jupyter-shaped output from `kernel_execute` (`KernelOutput` in Rust).
 * Variant tag is snake_case (`execute_result`); `display_data` is accepted
 * for forward compatibility even though the bridge does not emit it yet.
 */
export type NativeKernelOutput =
  | { type: "stream"; name: string; text: string }
  | { type: "execute_result"; data: Record<string, string> }
  | { type: "display_data"; data: Record<string, string> }
  | { type: "error"; ename: string; evalue: string; traceback: string[] };

/** Tauri `kernel_execute` result (`ExecuteResult` in Rust, camelCase). */
export type NativeExecuteResult = {
  requestId: string;
  status: string;
  outputs: NativeKernelOutput[];
};

function mimeTextPlain(data: Record<string, string>): string | null {
  const plain = data["text/plain"];
  return typeof plain === "string" && plain.length > 0 ? plain : null;
}

/**
 * Map a native `ExecuteResult` into the payload shape consumed by
 * `buildOutputsFromRun` / notebook merge.
 */
export function mapExecuteResultToKernelRunPayload(
  result: NativeExecuteResult,
): KernelRunPayload {
  let stdout = "";
  let stderr = "";
  let resultRepr: string | null = null;
  let error: KernelRunPayload["error"] = null;

  for (const output of result.outputs) {
    switch (output.type) {
      case "stream": {
        if (output.name === "stderr") {
          stderr += output.text;
        } else {
          // Jupyter may use "stdout" or other stream names; treat non-stderr as stdout.
          stdout += output.text;
        }
        break;
      }
      case "execute_result": {
        const plain = mimeTextPlain(output.data);
        if (plain != null) resultRepr = plain;
        break;
      }
      case "display_data": {
        // Prefer execute_result for resultRepr; use display_data only as fallback.
        if (resultRepr == null) {
          const plain = mimeTextPlain(output.data);
          if (plain != null) resultRepr = plain;
        }
        break;
      }
      case "error": {
        error = {
          ename: output.ename,
          evalue: output.evalue,
          traceback: output.traceback,
        };
        break;
      }
      default: {
        const unreachable: never = output;
        void unreachable;
        break;
      }
    }
  }

  return { stdout, stderr, resultRepr, error };
}

function abortError(): DOMException {
  return new DOMException("Notebook run cancelled", "AbortError");
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw abortError();
}

/**
 * Native desktop `KernelSession` over Tauri `kernel_*` commands.
 * Mount files / packages are ignored (no Pyodide FS); the bridge uses the
 * real workspace cwd.
 */
export function createNativeKernelSession(
  options: NativeKernelSessionOptions,
): KernelSession {
  const { root } = options;
  let sessionId: string | null = null;
  let disposed = false;
  let ensureInFlight: Promise<void> | null = null;

  const disposedError = (): Error =>
    new Error("Native kernel session has been disposed");

  const ensureSession = async (signal?: AbortSignal): Promise<void> => {
    if (disposed) throw disposedError();
    throwIfAborted(signal);
    if (sessionId != null) return;
    if (ensureInFlight) {
      await ensureInFlight;
      throwIfAborted(signal);
      if (sessionId == null && !disposed) {
        throw new Error("Native kernel session failed to start");
      }
      return;
    }

    ensureInFlight = (async () => {
      const response = await invoke<KernelStartResponse>("kernel_start", {
        request: { root },
      });
      if (disposed) {
        // Started after dispose raced — shut down immediately.
        try {
          await invoke("kernel_shutdown", {
            request: { sessionId: response.sessionId },
          });
        } catch {
          // Idempotent: unknown session is fine.
        }
        return;
      }
      sessionId = response.sessionId;
    })();

    try {
      await ensureInFlight;
    } finally {
      ensureInFlight = null;
    }

    throwIfAborted(signal);
    if (disposed) throw disposedError();
    if (sessionId == null) {
      throw new Error("Native kernel session failed to start");
    }
  };

  return {
    ensure(signal) {
      return ensureSession(signal);
    },

    async execute(code: string, executeOptions?: KernelExecuteOptions) {
      if (disposed) throw disposedError();
      // mountFiles / packages are intentionally ignored on native v1.
      void executeOptions?.mountFiles;
      void executeOptions?.packages;

      await ensureSession(executeOptions?.signal);
      throwIfAborted(executeOptions?.signal);
      if (sessionId == null) {
        throw new Error("Native kernel session is not started");
      }

      const result = await invoke<NativeExecuteResult>("kernel_execute", {
        request: { sessionId, code },
      });
      throwIfAborted(executeOptions?.signal);
      return mapExecuteResultToKernelRunPayload(result);
    },

    interrupt() {
      if (disposed || sessionId == null) return;
      const id = sessionId;
      void invoke("kernel_interrupt", { request: { sessionId: id } }).catch(() => {
        // Interrupt is best-effort; unknown / already-dead sessions are fine.
      });
    },

    dispose() {
      if (disposed) return;
      disposed = true;
      const id = sessionId;
      sessionId = null;
      if (id == null) return;
      void invoke("kernel_shutdown", { request: { sessionId: id } }).catch(() => {
        // Idempotent: ignore unknown session and other shutdown races.
      });
    },
  };
}
