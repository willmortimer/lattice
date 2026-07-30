import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const hasTauriState = { value: true };

vi.mock("./ipc", () => ({
  get hasTauri() {
    return hasTauriState.value;
  },
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { clearOpenaiApiKey, hasOpenaiApiKey, setOpenaiApiKey } from "./openaiKey";

describe("openaiKey helpers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    hasTauriState.value = true;
  });

  it("hasOpenaiApiKey returns false without Tauri", async () => {
    hasTauriState.value = false;
    await expect(hasOpenaiApiKey()).resolves.toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("hasOpenaiApiKey invokes presence command only", async () => {
    invokeMock.mockResolvedValue(true);
    await expect(hasOpenaiApiKey()).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("has_openai_api_key");
  });

  it("setOpenaiApiKey forwards key without echoing elsewhere", async () => {
    invokeMock.mockResolvedValue(undefined);
    await setOpenaiApiKey("sk-test");
    expect(invokeMock).toHaveBeenCalledWith("set_openai_api_key", { key: "sk-test" });
  });

  it("clearOpenaiApiKey invokes clear command", async () => {
    invokeMock.mockResolvedValue(undefined);
    await clearOpenaiApiKey();
    expect(invokeMock).toHaveBeenCalledWith("clear_openai_api_key");
  });

  it("setOpenaiApiKey rejects in browser demo", async () => {
    hasTauriState.value = false;
    await expect(setOpenaiApiKey("sk-test")).rejects.toThrow(/native desktop shell/);
  });
});
