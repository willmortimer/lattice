import { MAX_NOTEBOOK_OUTPUT_CHARS, PYODIDE_INDEX_URL } from "./pyodideConfig";
import type { PyodideRunPayload, PyodideWorkerRequest, PyodideWorkerResponse } from "./pyodideProtocol";

type PyodideInterface = {
  setStdout: (options: { batched: (text: string) => void }) => void;
  setStderr: (options: { batched: (text: string) => void }) => void;
  runPythonAsync: (code: string) => Promise<unknown>;
};

let pyodidePromise: Promise<PyodideInterface> | null = null;

function cap(text: string, maxChars: number): string {
  if (text.length <= maxChars) return text;
  return `${text.slice(0, maxChars)}\n… [output truncated at ${maxChars} characters]`;
}

async function loadPyodideRuntime(): Promise<PyodideInterface> {
  const moduleUrl = `${PYODIDE_INDEX_URL}pyodide.mjs`;
  // CDN import keeps the desktop bundle free of the ~10–15 MB Pyodide assets.
  const { loadPyodide } = await import(/* @vite-ignore */ moduleUrl) as {
    loadPyodide: (options: { indexURL: string }) => Promise<PyodideInterface>;
  };
  return loadPyodide({ indexURL: PYODIDE_INDEX_URL });
}

function ensurePyodide(): Promise<PyodideInterface> {
  if (!pyodidePromise) {
    pyodidePromise = loadPyodideRuntime().catch((error) => {
      pyodidePromise = null;
      throw error;
    });
  }
  return pyodidePromise;
}

function appendBatch(buffer: { value: string }, text: string): void {
  buffer.value += text.endsWith("\n") ? text : `${text}\n`;
}

async function runCell(
  pyodide: PyodideInterface,
  code: string,
  maxOutputChars: number,
): Promise<PyodideRunPayload> {
  const stdout = { value: "" };
  const stderr = { value: "" };
  pyodide.setStdout({ batched: (text) => appendBatch(stdout, text) });
  pyodide.setStderr({ batched: (text) => appendBatch(stderr, text) });

  // Evaluate the last expression (notebook-style) while keeping statements exec'd.
  const runner = `
import ast as __lt_ast
import traceback as __lt_traceback

__lt_source = ${JSON.stringify(code)}
__lt_payload = {"repr": None, "error": None}
try:
    __lt_tree = __lt_ast.parse(__lt_source, mode="exec")
    __lt_result = None
    if __lt_tree.body and isinstance(__lt_tree.body[-1], __lt_ast.Expr):
        __lt_expr = __lt_tree.body.pop()
        exec(compile(__lt_ast.Module(__lt_tree.body, type_ignores=[]), "<cell>", "exec"), globals())
        __lt_result = eval(compile(__lt_ast.Expression(__lt_expr.value), "<cell>", "eval"), globals())
    else:
        exec(compile(__lt_tree, "<cell>", "exec"), globals())
    if __lt_result is not None:
        __lt_payload["repr"] = repr(__lt_result)
except Exception as __lt_exc:
    __lt_payload["error"] = {
        "ename": type(__lt_exc).__name__,
        "evalue": str(__lt_exc),
        "traceback": __lt_traceback.format_exception(type(__lt_exc), __lt_exc, __lt_exc.__traceback__),
    }
__lt_payload
`;

  const payloadProxy = await pyodide.runPythonAsync(runner);
  const payload = (payloadProxy && typeof payloadProxy === "object" && "toJs" in payloadProxy
    ? (payloadProxy as { toJs: (opts: { dict_converter: typeof Object.fromEntries }) => Record<string, unknown> })
      .toJs({ dict_converter: Object.fromEntries })
    : payloadProxy) as {
    repr?: string | null;
    error?: { ename?: string; evalue?: string; traceback?: string[] } | null;
  };

  const error = payload?.error
    ? {
        ename: String(payload.error.ename ?? "Error"),
        evalue: String(payload.error.evalue ?? ""),
        traceback: Array.isArray(payload.error.traceback)
          ? payload.error.traceback.map((line) => cap(String(line), maxOutputChars))
          : [],
      }
    : null;

  return {
    stdout: cap(stdout.value, maxOutputChars),
    stderr: cap(stderr.value, maxOutputChars),
    resultRepr: payload?.repr != null ? cap(String(payload.repr), maxOutputChars) : null,
    error,
  };
}

self.onmessage = async (event: MessageEvent<PyodideWorkerRequest>) => {
  const request = event.data;
  try {
    if (request.type === "ensure") {
      await ensurePyodide();
      const response: PyodideWorkerResponse = { type: "ready", id: request.id };
      self.postMessage(response);
      return;
    }

    if (request.type === "run") {
      const pyodide = await ensurePyodide();
      const payload = await runCell(
        pyodide,
        request.code,
        request.maxOutputChars ?? MAX_NOTEBOOK_OUTPUT_CHARS,
      );
      const response: PyodideWorkerResponse = { type: "result", id: request.id, payload };
      self.postMessage(response);
      return;
    }

    const unreachable: never = request;
    void unreachable;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const response: PyodideWorkerResponse = request.type === "ensure" || pyodidePromise === null
      ? { type: "load-error", id: request.id, message }
      : { type: "error", id: request.id, message };
    self.postMessage(response);
  }
};
