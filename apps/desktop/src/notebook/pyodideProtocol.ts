export type PyodideWorkerRequest =
  | { type: "ensure"; id: number }
  | { type: "run"; id: number; code: string; maxOutputChars?: number };

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
