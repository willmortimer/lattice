import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../lib/appLock", () => ({
  unlockApp: vi.fn(async () => ({
    enabled: true,
    locked: false,
    idleLockMinutes: 5,
    presenceAvailable: true,
    platformSupported: true,
  })),
}));

import { AppLockOverlay } from "./AppLockOverlay";

describe("AppLockOverlay", () => {
  it("renders unlock affordance", () => {
    render(<AppLockOverlay onUnlocked={() => undefined} />);
    expect(screen.getByRole("dialog", { name: "Lattice is locked" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /unlock/i })).toBeTruthy();
    expect(screen.getByText(/session locked/i)).toBeTruthy();
  });
});
