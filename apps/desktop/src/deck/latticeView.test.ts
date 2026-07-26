import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import {
  MAX_DECK_VIEWBOX_IMAGE_BYTES,
  MAX_DECK_VIEWBOX_IMAGE_TOTAL_BYTES,
  parseLatticeViewRequest,
} from "./latticeView";

function attributes(values: Record<string, string | undefined>) {
  return { getAttribute: (name: string) => values[name] ?? null };
}

describe("lattice-view source contract", () => {
  it("requires a resource and keeps live as an explicit deferred mode", () => {
    expect(parseLatticeViewRequest(attributes({ resource: "Notes/Plan.md", mode: "live" }))).toEqual({
      resource: "Notes/Plan.md",
      mode: "live",
    });
    expect(parseLatticeViewRequest(attributes({ resource: "Notes/Plan.md", mode: "anything" }))).toEqual({
      resource: "Notes/Plan.md",
      mode: "static",
    });
    expect(parseLatticeViewRequest(attributes({ mode: "live" }))).toBeNull();
  });

  it("publishes bounded raster snapshot budgets", () => {
    expect(MAX_DECK_VIEWBOX_IMAGE_BYTES).toBe(8 * 1024 * 1024);
    expect(MAX_DECK_VIEWBOX_IMAGE_TOTAL_BYTES).toBe(32 * 1024 * 1024);
  });
});
