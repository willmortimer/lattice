import { describe, expect, it, afterEach } from "vitest";

import {
  clearAnchorAdapters,
  getAnchorAdapter,
  getAnchorAdapterFor,
  registerAnchorAdapter,
} from "./registry";
import type { AgentAnchorAdapter } from "./types";

const markdownAdapter: AgentAnchorAdapter = {
  kind: "markdown-block",
  resourceId: "Notes/Page.md",
  reveal: async () => undefined,
  highlight: () => () => undefined,
};

const datasetAdapter: AgentAnchorAdapter = {
  kind: "dataset-region",
  resourceId: "Tables/People.data",
  reveal: async () => undefined,
  highlight: () => () => undefined,
};

describe("anchor adapter registry", () => {
  afterEach(() => {
    clearAnchorAdapters();
  });

  it("registers and looks up adapters by surface kind", () => {
    registerAnchorAdapter(markdownAdapter);
    registerAnchorAdapter(datasetAdapter);
    expect(getAnchorAdapter("markdown-block")).toBe(markdownAdapter);
    expect(getAnchorAdapter("dataset-region")).toBe(datasetAdapter);
  });

  it("unregisters adapters and scopes lookups by resource id", () => {
    const unregister = registerAnchorAdapter(markdownAdapter);
    expect(
      getAnchorAdapterFor({
        kind: "markdown-block",
        resourceId: "Notes/Page.md",
        blockId: "root|paragraph#0",
      }),
    ).toBe(markdownAdapter);
    expect(
      getAnchorAdapterFor({
        kind: "markdown-block",
        resourceId: "Other.md",
        blockId: "root|paragraph#0",
      }),
    ).toBeUndefined();
    unregister();
    expect(getAnchorAdapter("markdown-block")).toBeUndefined();
  });
});
