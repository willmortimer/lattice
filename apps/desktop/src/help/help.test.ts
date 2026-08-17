// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

import { getGuidanceAnchor, registerGuidanceAnchor } from "../guidance/registry";
import type { GuidanceAnchor } from "../guidance/types";
import {
  buildHelpCorpus,
  filterHelpPages,
  findHelpPageByStem,
  parseHelpNavigation,
  parseHelpPageRaw,
  stemFromHelpFile,
} from "./helpCorpus";
import { parseHelpDeepLinkUrl } from "./helpDeepLink";
import { HELP_NAVIGATION, HELP_RAW_BY_FILE } from "./helpManifest";

const CORPUS = buildHelpCorpus(HELP_NAVIGATION, HELP_RAW_BY_FILE);

describe("help navigation parse", () => {
  it("parses navigation sections and builds pages from manifest", () => {
    const navigation = parseHelpNavigation(HELP_NAVIGATION);
    expect(navigation.length).toBeGreaterThan(0);
    expect(navigation[0]?.items[0]?.file).toMatch(/\.md$/);
    expect(CORPUS.pages.length).toBe(13);
    expect(stemFromHelpFile("find-and-jump.md")).toBe("find-and-jump");
  });

  it("parses optional anchor frontmatter", () => {
    const page = parseHelpPageRaw(
      "tour.md",
      "---\ntitle: Tour\nanchor: shell.search\n---\n\nBody",
      "Tour",
    );
    expect(page.anchor).toBe("shell.search");
  });
});

describe("help search filter", () => {
  it("filters pages by title and body text", () => {
    const matches = filterHelpPages(CORPUS.pages, "spreadsheet");
    expect(matches.some((page) => page.stem === "import-csv")).toBe(true);
    const agentMatches = filterHelpPages(CORPUS.pages, "agent");
    expect(agentMatches.some((page) => page.stem === "agent")).toBe(true);
    const clipMatches = filterHelpPages(CORPUS.pages, "clip");
    expect(clipMatches.some((page) => page.stem === "capture")).toBe(true);
    expect(filterHelpPages(CORPUS.pages, "zzzz-not-in-help")).toHaveLength(0);
  });
});

describe("help deep link stem", () => {
  it("parses lattice help URLs to file stems", () => {
    expect(parseHelpDeepLinkUrl("lattice://help/inspect")).toBe("inspect");
    expect(parseHelpDeepLinkUrl("lattice://help/find-and-jump")).toBe("find-and-jump");
    expect(parseHelpDeepLinkUrl("#help/welcome")).toBe("welcome");
    expect(parseHelpDeepLinkUrl("lattice://settings/ai/provider")).toBeNull();
  });
});

describe("help Show me", () => {
  it("calls reveal on a registered guidance anchor", async () => {
    const reveal = vi.fn(async () => undefined);
    const anchor: GuidanceAnchor = {
      id: "test.help.anchor",
      isAvailable: () => true,
      reveal,
      getRect: () => null,
    };
    const unregister = registerGuidanceAnchor(anchor);
    const resolved = getGuidanceAnchor("test.help.anchor");
    expect(resolved).toBeDefined();
    await resolved?.reveal();
    expect(reveal).toHaveBeenCalledOnce();
    unregister();
  });
});
