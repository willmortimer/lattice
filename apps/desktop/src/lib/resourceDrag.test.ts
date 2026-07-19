import { describe, expect, it } from "vitest";

import {
  decodeResourceDragPayload,
  encodeResourceDragPayload,
  hasLatticeResourceDrag,
  latticeEmbedMarkdown,
  LATTICE_RESOURCE_MIME,
  pageDropIntent,
  wikiLinkMarkdown,
} from "./resourceDrag";
import type { Resource } from "../types";

const sample: Resource = {
  path: "Notes/Spec.md",
  kind: "page",
  formatId: "markdown",
};

describe("resourceDrag payloads", () => {
  it("round-trips sidebar resource payloads", () => {
    const encoded = encodeResourceDragPayload(sample);
    expect(decodeResourceDragPayload(encoded)).toEqual({
      version: 1,
      path: "Notes/Spec.md",
      kind: "page",
      formatId: "markdown",
      title: "Spec.md",
    });
  });

  it("rejects malformed payloads", () => {
    expect(decodeResourceDragPayload("{")).toBeNull();
    expect(decodeResourceDragPayload(JSON.stringify({ version: 1 }))).toBeNull();
  });

  it("builds link vs embed markdown and respects Alt for embed intent", () => {
    const payload = decodeResourceDragPayload(encodeResourceDragPayload(sample))!;
    expect(wikiLinkMarkdown(payload)).toBe("[[Spec.md]]");
    expect(latticeEmbedMarkdown(payload)).toContain("resource: Notes/Spec.md");
    expect(pageDropIntent({ altKey: false })).toBe("link");
    expect(pageDropIntent({ altKey: true })).toBe("embed");
  });

  it("detects Lattice resource drags from types alone (dragover-safe)", () => {
    const transfer = {
      types: [LATTICE_RESOURCE_MIME, "text/plain"],
      getData: () => "",
    } as unknown as DataTransfer;
    expect(hasLatticeResourceDrag(transfer)).toBe(true);
    expect(hasLatticeResourceDrag({ types: ["text/plain"] } as unknown as DataTransfer)).toBe(false);
    expect(hasLatticeResourceDrag(null)).toBe(false);
  });
});
