import { describe, expect, it } from "vitest";
import { createResourceLoadGate } from "./resourceLoad";

describe("resource load cancellation", () => {
  it("aborts an older load and accepts only the newest ticket", () => {
    const gate = createResourceLoadGate();
    const first = gate.begin();
    const second = gate.begin();

    expect(first.controller.signal.aborted).toBe(true);
    expect(gate.isCurrent(first)).toBe(false);
    expect(gate.isCurrent(second)).toBe(true);
  });

  it("invalidates the active ticket when cancelled explicitly", () => {
    const gate = createResourceLoadGate();
    const ticket = gate.begin();
    gate.cancel();
    expect(ticket.controller.signal.aborted).toBe(true);
    expect(gate.isCurrent(ticket)).toBe(false);
  });
});
