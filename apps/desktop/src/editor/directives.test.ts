import { describe, expect, it } from "vitest";

import {
  latticeEmbedAttrsFromFields,
  parseDirectiveBody,
  parseEmbedMode,
  serializeLatticeEmbed,
} from "./directives";

describe("parseEmbedMode", () => {
  it("defaults to card", () => {
    expect(parseEmbedMode(null)).toBe("card");
    expect(parseEmbedMode("")).toBe("card");
    expect(parseEmbedMode("nope")).toBe("card");
  });

  it("accepts card, preview, and interactive", () => {
    expect(parseEmbedMode("card")).toBe("card");
    expect(parseEmbedMode("preview")).toBe("preview");
    expect(parseEmbedMode("interactive")).toBe("interactive");
    expect(parseEmbedMode(" Interactive ")).toBe("interactive");
  });
});

describe("lattice-embed mode field", () => {
  it("parses mode from directive body", () => {
    const fields = parseDirectiveBody("resource: Artifacts/Pulse.artifact\nmode: interactive\n");
    const attrs = latticeEmbedAttrsFromFields(fields);
    expect(attrs.mode).toBe("interactive");
    expect(attrs.resource).toBe("Artifacts/Pulse.artifact");
  });

  it("round-trips mode in serialization order", () => {
    const markdown = serializeLatticeEmbed({
      resource: "Artifacts/Pulse.artifact",
      view: null,
      mode: "interactive",
      height: "320",
      lines: null,
      fallback: null,
      extraFields: {},
      extraFieldKeys: [],
    });
    expect(markdown).toContain("resource: Artifacts/Pulse.artifact\n");
    expect(markdown).toContain("mode: interactive\n");
    expect(markdown).toContain("height: 320\n");
    expect(markdown.indexOf("mode:")).toBeLessThan(markdown.indexOf("height:"));
  });
});
