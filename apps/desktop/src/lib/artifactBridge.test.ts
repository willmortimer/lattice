import { describe, expect, it } from "vitest";

import {
  isArtifactFrameMessage,
  isArtifactHostMessage,
  type ArtifactFrameToHostMessage,
  type ArtifactHostToFrameMessage,
} from "./artifactBridge";
import type { BindingSpec } from "./bindingSpec";

const binding: BindingSpec = {
  type: "sqlite-query",
  resource: "CRM.data",
  sql: "SELECT COUNT(*) AS value FROM contacts",
  limit: 1,
};

describe("artifactBridge message schema", () => {
  it("accepts frame → host requestBinding / openResource / notify", () => {
    const messages: ArtifactFrameToHostMessage[] = [
      { type: "lattice.artifact.requestBinding", id: "1", name: "contactCount" },
      { type: "lattice.artifact.openResource", path: "CRM.data" },
      { type: "lattice.artifact.notify", title: "Pulse", height: 280 },
    ];
    for (const message of messages) {
      expect(isArtifactFrameMessage(message)).toBe(true);
    }
  });

  it("rejects malformed frame messages", () => {
    expect(isArtifactFrameMessage({ type: "lattice.artifact.requestBinding" })).toBe(false);
    expect(isArtifactFrameMessage({ type: "other", id: "1", name: "x" })).toBe(false);
    expect(isArtifactFrameMessage(null)).toBe(false);
  });

  it("accepts host → frame init / theme / bindingResult", () => {
    const messages: ArtifactHostToFrameMessage[] = [
      { type: "lattice.artifact.init", title: "Pulse", bindings: ["contactCount"] },
      {
        type: "lattice.artifact.theme",
        vars: { "--lt-bg": "#fff" },
        background: "#fff",
        appearance: "light",
      },
      {
        type: "lattice.artifact.bindingResult",
        id: "1",
        ok: true,
        data: { kind: "scalar", column: "value", value: 20, binding },
      },
    ];
    for (const message of messages) {
      expect(isArtifactHostMessage(message)).toBe(true);
    }
  });

  it("rejects host messages with non-string theme vars", () => {
    expect(
      isArtifactHostMessage({
        type: "lattice.artifact.theme",
        vars: { "--lt-bg": 12 },
      }),
    ).toBe(false);
  });
});
