import { describe, expect, it } from "vitest";

import { defaultDesktopSettings } from "./profile";

describe("defaultDesktopSettings ai", () => {
  it("defaults to local mode with followAi embeddings", () => {
    const settings = defaultDesktopSettings();
    expect(settings.ai).toEqual({
      mode: "local",
      embeddingMode: "followAi",
      passiveEmbeddingEnabled: false,
      preferredModel: null,
    });
  });
});
