import { describe, expect, it } from "vitest";

import {
  BROWSER_UNDO_UNAVAILABLE_MESSAGE,
  browserUndoBlocked,
} from "./browserUndoGuard";

describe("browserUndoBlocked", () => {
  it("blocks workspace undo in the browser demo fixture", () => {
    expect(browserUndoBlocked(true)).toBe(true);
  });

  it("allows workspace undo in native and bridge shells", () => {
    expect(browserUndoBlocked(false)).toBe(false);
  });

  it("exposes a stable toast message for the guard", () => {
    expect(BROWSER_UNDO_UNAVAILABLE_MESSAGE).toMatch(/browser demo/i);
  });
});
