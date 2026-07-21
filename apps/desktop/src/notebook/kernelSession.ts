import type { PyodideMountFile, PyodideRunPayload } from "./pyodideProtocol";

/** Execute result shape shared with `mergeNotebookOutputs.buildOutputsFromRun`. */
export type KernelRunPayload = PyodideRunPayload;

export type KernelMountFile = PyodideMountFile;

export type KernelExecuteOptions = {
  signal?: AbortSignal;
  maxOutputChars?: number;
  /** Files to write into the kernel FS before executing the cell. */
  mountFiles?: KernelMountFile[];
  /** Packages to load before the cell (backend-specific; cached when supported). */
  packages?: string[];
};

/**
 * Frontend notebook execution session (Phase-4 local contract).
 * Backends implement ensure / execute / interrupt / dispose; Pyodide is one.
 */
export interface KernelSession {
  /** Lazily start / reconnect the session; idempotent. */
  ensure(signal?: AbortSignal): Promise<void>;
  /** Run one cell; return a payload mergeable via `buildOutputsFromRun`. */
  execute(code: string, options?: KernelExecuteOptions): Promise<KernelRunPayload>;
  /** Cancel in-flight execution when the backend supports it. */
  interrupt(): void;
  /** Tear down the session; safe to call more than once. */
  dispose(): void;
}
