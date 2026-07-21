import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  createNativeKernelSession,
  mapExecuteResultToKernelRunPayload,
  type NativeExecuteResult,
} from "./nativeKernelSession";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("mapExecuteResultToKernelRunPayload", () => {
  it("maps stdout, stderr, execute_result, and error outputs", () => {
    const result: NativeExecuteResult = {
      requestId: "r1",
      status: "error",
      outputs: [
        { type: "stream", name: "stdout", text: "hello\n" },
        { type: "stream", name: "stderr", text: "warn\n" },
        {
          type: "execute_result",
          data: { "text/plain": "42" },
        },
        {
          type: "error",
          ename: "ValueError",
          evalue: "boom",
          traceback: ["Traceback...", "ValueError: boom"],
        },
      ],
    };

    expect(mapExecuteResultToKernelRunPayload(result)).toEqual({
      stdout: "hello\n",
      stderr: "warn\n",
      resultRepr: "42",
      error: {
        ename: "ValueError",
        evalue: "boom",
        traceback: ["Traceback...", "ValueError: boom"],
      },
    });
  });

  it("concatenates multiple streams and prefers execute_result over display_data", () => {
    const result: NativeExecuteResult = {
      requestId: "r2",
      status: "ok",
      outputs: [
        { type: "stream", name: "stdout", text: "a" },
        { type: "stream", name: "stdout", text: "b" },
        { type: "display_data", data: { "text/plain": "display" } },
        { type: "execute_result", data: { "text/plain": "exec" } },
      ],
    };

    expect(mapExecuteResultToKernelRunPayload(result)).toEqual({
      stdout: "ab",
      stderr: "",
      resultRepr: "exec",
      error: null,
    });
  });

  it("uses display_data text/plain when execute_result is absent", () => {
    const result: NativeExecuteResult = {
      requestId: "r3",
      status: "ok",
      outputs: [{ type: "display_data", data: { "text/plain": "shown" } }],
    };

    expect(mapExecuteResultToKernelRunPayload(result)).toEqual({
      stdout: "",
      stderr: "",
      resultRepr: "shown",
      error: null,
    });
  });
});

describe("createNativeKernelSession", () => {
  afterEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("starts once, executes with mapped payload, and shuts down on dispose", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "kernel_start":
          return { sessionId: "k-1" };
        case "kernel_execute":
          return {
            requestId: "req-1",
            status: "ok",
            outputs: [
              { type: "stream", name: "stdout", text: "1\n" },
              { type: "execute_result", data: { "text/plain": "1" } },
            ],
          } satisfies NativeExecuteResult;
        case "kernel_shutdown":
          return undefined;
        default:
          throw new Error(`unexpected command ${cmd}`);
      }
    });

    const session = createNativeKernelSession({ root: "/ws" });
    await session.ensure();
    await session.ensure();
    const payload = await session.execute("print(1)", {
      packages: ["pandas"],
      mountFiles: [{ mountPath: "/x", data: new Uint8Array([1]) }],
    });

    expect(payload).toEqual({
      stdout: "1\n",
      stderr: "",
      resultRepr: "1",
      error: null,
    });
    expect(invoke).toHaveBeenCalledWith("kernel_start", {
      request: { root: "/ws" },
    });
    expect(invoke).toHaveBeenCalledWith("kernel_execute", {
      request: { sessionId: "k-1", code: "print(1)" },
    });
    expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "kernel_start")).toHaveLength(1);

    session.dispose();
    session.dispose();
    expect(invoke).toHaveBeenCalledWith("kernel_shutdown", {
      request: { sessionId: "k-1" },
    });
    await expect(session.execute("1")).rejects.toThrow(/disposed/);
  });

  it("interrupt invokes kernel_interrupt for a live session", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "kernel_start") return { sessionId: "k-2" };
      if (cmd === "kernel_interrupt") return undefined;
      throw new Error(`unexpected ${cmd}`);
    });

    const session = createNativeKernelSession({ root: "/ws" });
    await session.ensure();
    session.interrupt();
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith("kernel_interrupt", {
      request: { sessionId: "k-2" },
    });
  });
});
