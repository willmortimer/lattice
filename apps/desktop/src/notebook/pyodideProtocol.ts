export type PyodideMountFile = {
  /** Absolute path inside the Pyodide FS (e.g. `/home/pyodide/workspace/...`). */
  mountPath: string;
  /** File bytes to write (copied into the worker). */
  data: Uint8Array;
};

export type PyodideWorkerRequest =
  | { type: "ensure"; id: number }
  | {
      type: "run";
      id: number;
      code: string;
      maxOutputChars?: number;
      mountFiles?: PyodideMountFile[];
      packages?: string[];
    };

export type PyodideRunPayload = {
  stdout: string;
  stderr: string;
  resultRepr: string | null;
  error: { ename: string; evalue: string; traceback: string[] } | null;
};

export type PyodideWorkerResponse =
  | { type: "ready"; id: number }
  | { type: "result"; id: number; payload: PyodideRunPayload }
  | { type: "load-error"; id: number; message: string }
  | { type: "error"; id: number; message: string };
