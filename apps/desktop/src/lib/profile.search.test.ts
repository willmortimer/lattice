import { describe, expect, it } from "vitest";

import { defaultDesktopSettings } from "./profile";

describe("defaultDesktopSettings search", () => {
  it("defaults semantic search off", () => {
    const settings = defaultDesktopSettings();
    expect(settings.search.semanticEnabled).toBe(false);
  });
});
