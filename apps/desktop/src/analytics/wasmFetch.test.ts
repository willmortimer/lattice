import { describe, expect, it, vi } from "vitest";

import { fetchWasmBytes } from "./wasmFetch";

describe("wasmFetch", () => {
  it("rejects non-WASM fetch bodies before WebAssembly.compile", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      arrayBuffer: async () => new TextEncoder().encode("<!DOCTYPE html>").buffer,
    });
    vi.stubGlobal("fetch", fetchMock);

    await expect(fetchWasmBytes("/fake.wasm")).rejects.toThrow(/Invalid WASM/);
    vi.unstubAllGlobals();
  });

  it("accepts buffers with the \\0asm magic", async () => {
    const bytes = new Uint8Array(16);
    bytes[0] = 0x00;
    bytes[1] = 0x61;
    bytes[2] = 0x73;
    bytes[3] = 0x6d;
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      arrayBuffer: async () => bytes.buffer,
    });
    vi.stubGlobal("fetch", fetchMock);

    const buffer = await fetchWasmBytes("/ok.wasm");
    expect(new Uint8Array(buffer)[1]).toBe(0x61);
    vi.unstubAllGlobals();
  });
});
