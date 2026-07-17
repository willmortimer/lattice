import { afterEach, describe, expect, it, vi } from "vitest";

import {
  BRIDGE_COMMANDS,
  isBridgeCommand,
  postBridgeCommand,
  resolveIpcMode,
} from "./ipc";

describe("resolveIpcMode", () => {
  it("prefers Tauri when the internals bridge is present", () => {
    expect(resolveIpcMode(true, "http://127.0.0.1:8787")).toBe("tauri");
  });

  it("uses bridge when no Tauri and bridge URL is set", () => {
    expect(resolveIpcMode(false, "http://127.0.0.1:8787")).toBe("bridge");
  });

  it("falls back to demo without Tauri or bridge URL", () => {
    expect(resolveIpcMode(false, undefined)).toBe("demo");
    expect(resolveIpcMode(false, "   ")).toBe("demo");
  });
});

describe("isBridgeCommand", () => {
  it("includes MVP handler routes", () => {
    expect(BRIDGE_COMMANDS.has("read_page")).toBe(true);
    expect(isBridgeCommand("search_workspace")).toBe(true);
  });

  it("excludes Tauri-only commands", () => {
    expect(isBridgeCommand("start_watching")).toBe(false);
  });
});

describe("postBridgeCommand", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("POSTs camelCase JSON to the handler route", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => JSON.stringify({ content: "hello", revision: "1" }),
    });

    const result = await postBridgeCommand<{ content: string; revision: string }>(
      "http://127.0.0.1:8787",
      "read_page",
      { root: "/ws", relPath: "Home.md" },
      fetchImpl,
    );

    expect(fetchImpl).toHaveBeenCalledWith(
      "http://127.0.0.1:8787/read_page",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ root: "/ws", relPath: "Home.md" }),
      }),
    );
    expect(result).toEqual({ content: "hello", revision: "1" });
  });

  it("unwraps revision objects for page write commands", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => JSON.stringify({ revision: "rev-2" }),
    });

    const revision = await postBridgeCommand<string>(
      "http://127.0.0.1:8787/",
      "apply_page_update",
      { relPath: "a.md", content: "x", baseRevision: "1" },
      fetchImpl,
    );

    expect(revision).toBe("rev-2");
  });

  it("prefixes stale revision conflicts like Tauri", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: false,
      status: 409,
      statusText: "Conflict",
      text: async () => JSON.stringify({ error: "STALE_REVISION:disk changed" }),
    });

    await expect(
      postBridgeCommand("http://127.0.0.1:8787", "apply_page_update", {}, fetchImpl),
    ).rejects.toBe("STALE_REVISION:disk changed");
  });

  it("rejects non-MVP commands", async () => {
    await expect(
      postBridgeCommand("http://127.0.0.1:8787", "start_watching", {}, vi.fn()),
    ).rejects.toThrow(/not available in bridge mode/);
  });
});
