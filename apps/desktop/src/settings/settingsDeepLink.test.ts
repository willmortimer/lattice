import { describe, expect, it } from "vitest";

import { resolveSettingsDeepLink } from "./settingsCatalog";
import { parseSettingsDeepLinkUrl } from "./settingsDeepLink";

describe("parseSettingsDeepLinkUrl", () => {
  it("parses lattice scheme paths", () => {
    expect(parseSettingsDeepLinkUrl("lattice://settings/ai/provider")).toBe("ai/provider");
    expect(parseSettingsDeepLinkUrl("lattice://settings/search/semantic")).toBe(
      "search/semantic",
    );
    expect(parseSettingsDeepLinkUrl("lattice://settings/remote-access")).toBe("remote-access");
  });

  it("parses hash fragments for browser demo", () => {
    expect(parseSettingsDeepLinkUrl("#settings/search/semantic")).toBe("search/semantic");
  });
});

describe("resolveSettingsDeepLink", () => {
  it("resolves documented aliases", () => {
    expect(resolveSettingsDeepLink("ai/provider")).toEqual({
      section: "ai",
      settingId: "ai.mode",
    });
    expect(resolveSettingsDeepLink("search/semantic")).toEqual({
      section: "search",
      settingId: "search.semantic",
    });
    expect(resolveSettingsDeepLink("remote-access")).toEqual({
      section: "remote",
      settingId: "remote.access",
    });
  });

  it("resolves section-only paths", () => {
    expect(resolveSettingsDeepLink("appearance")).toEqual({
      section: "appearance",
      settingId: null,
    });
  });
});
