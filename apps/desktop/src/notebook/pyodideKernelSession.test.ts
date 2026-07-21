import { afterEach, describe, expect, it, vi } from "vitest";
import { createPyodideKernelSession } from "./pyodideKernelSession";
import {
  ensurePyodide,
  PyodideCancelledError,
  PyodideLoadError,
  resetPyodideRuntime,
  runPythonCell,
} from "./pyodideRuntime";

vi.mock("./pyodideRuntime", () => ({
  ensurePyodide: vi.fn(),
  runPythonCell: vi.fn(),
  resetPyodideRuntime: vi.fn(),
  PyodideLoadError: class PyodideLoadError extends Error {
    constructor(message: string) {
      super(message);
      this.name = "PyodideLoadError";
    }
  },
  PyodideCancelledError: class PyodideCancelledError extends Error {
    constructor() {
      super("Notebook run cancelled");
      this.name = "PyodideCancelledError";
    }
  },
}));

describe("createPyodideKernelSession", () => {
  afterEach(() => {
    vi.mocked(ensurePyodide).mockReset();
    vi.mocked(runPythonCell).mockReset();
    vi.mocked(resetPyodideRuntime).mockReset();
  });

  it("forwards ensure and execute to the Pyodide runtime", async () => {
    const session = createPyodideKernelSession();
    const signal = new AbortController().signal;
    vi.mocked(ensurePyodide).mockResolvedValueOnce(undefined);
    vi.mocked(runPythonCell).mockResolvedValueOnce({
      stdout: "ok",
      stderr: "",
      resultRepr: null,
      error: null,
    });

    await session.ensure(signal);
    const payload = await session.execute("print(1)", {
      signal,
      packages: ["pandas"],
      mountFiles: [{ mountPath: "/tmp/x", data: new Uint8Array([1]) }],
    });

    expect(ensurePyodide).toHaveBeenCalledWith(signal);
    expect(runPythonCell).toHaveBeenCalledWith("print(1)", {
      signal,
      packages: ["pandas"],
      mountFiles: [{ mountPath: "/tmp/x", data: new Uint8Array([1]) }],
    });
    expect(payload.stdout).toBe("ok");
  });

  it("propagates load and cancel errors from execute", async () => {
    const session = createPyodideKernelSession();
    vi.mocked(runPythonCell).mockRejectedValueOnce(new PyodideLoadError("cdn down"));
    await expect(session.execute("1")).rejects.toBeInstanceOf(PyodideLoadError);

    vi.mocked(runPythonCell).mockRejectedValueOnce(new PyodideCancelledError());
    await expect(session.execute("1")).rejects.toBeInstanceOf(PyodideCancelledError);
  });

  it("interrupt and dispose tear down the worker; dispose is idempotent", () => {
    const session = createPyodideKernelSession();
    session.interrupt();
    expect(resetPyodideRuntime).toHaveBeenCalledTimes(1);

    session.dispose();
    session.dispose();
    expect(resetPyodideRuntime).toHaveBeenCalledTimes(2);
  });

  it("rejects ensure/execute after dispose", async () => {
    const session = createPyodideKernelSession();
    session.dispose();
    await expect(session.ensure()).rejects.toThrow(/disposed/);
    await expect(session.execute("1")).rejects.toThrow(/disposed/);
    session.interrupt();
  });
});
