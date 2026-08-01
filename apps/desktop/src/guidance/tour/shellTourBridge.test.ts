// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

import { requestShellTourStart, subscribeShellTourStart } from "./shellTourBridge";

describe("shellTourBridge", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("dispatches a window event to request the shell tour", () => {
    const listener = vi.fn();
    const unsubscribe = subscribeShellTourStart(listener);
    requestShellTourStart();
    expect(listener).toHaveBeenCalledTimes(1);
    unsubscribe();
  });
});
