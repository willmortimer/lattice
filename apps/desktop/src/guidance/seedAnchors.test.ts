// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

import { clearGuidanceAnchors } from "./registry";
import { createDefaultGuidanceAnchors } from "./seedAnchors";

describe("seedAnchors", () => {
  afterEach(() => {
    clearGuidanceAnchors();
    document.body.innerHTML = "";
  });

  it("reveals settings.ai.provider via the settings deep link", async () => {
    const anchor = createDefaultGuidanceAnchors().find((item) => item.id === "settings.ai.provider");
    expect(anchor).toBeDefined();

    const target = document.createElement("div");
    target.setAttribute("data-guidance-anchor", "settings.ai.provider");
    target.scrollIntoView = () => {};
    target.getBoundingClientRect = () => new DOMRect(0, 0, 200, 80) as DOMRect;
    document.body.append(target);

    const deepLinkSpy = vi.spyOn(window, "dispatchEvent");
    await anchor!.reveal();
    expect(deepLinkSpy).toHaveBeenCalled();
    const event = deepLinkSpy.mock.calls[0]![0] as CustomEvent;
    expect(event.type).toBe("lattice:settings-deeplink");
    expect(event.detail).toEqual({ section: "ai", settingId: "ai.mode" });
  });
});
