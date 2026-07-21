import type { KernelExecuteOptions, KernelSession } from "./kernelSession";
import {
  ensurePyodide,
  resetPyodideRuntime,
  runPythonCell,
} from "./pyodideRuntime";

/**
 * Wrap the existing Pyodide worker runtime as a `KernelSession`.
 * Cancel still terminates the worker via AbortSignal / `resetPyodideRuntime`.
 */
export function createPyodideKernelSession(): KernelSession {
  let disposed = false;

  const disposedError = (): Error =>
    new Error("Pyodide kernel session has been disposed");

  return {
    ensure(signal) {
      if (disposed) return Promise.reject(disposedError());
      return ensurePyodide(signal);
    },

    execute(code: string, options?: KernelExecuteOptions) {
      if (disposed) return Promise.reject(disposedError());
      return runPythonCell(code, options);
    },

    interrupt() {
      if (disposed) return;
      resetPyodideRuntime();
    },

    dispose() {
      if (disposed) return;
      disposed = true;
      resetPyodideRuntime();
    },
  };
}
