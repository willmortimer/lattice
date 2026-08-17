import { describe, expect, it } from "vitest";
import {
  formatAuthority,
  formatMaterialization,
  formatResourceAuthority,
  persistModeFromResourceAuthority,
  persistModeFromResourceStat,
  resourceAuthorityForPersistMode,
  type ResourceStat,
} from "./resourceStat";

describe("formatAuthority", () => {
  it("labels cloud authority for Inspect Properties", () => {
    expect(formatAuthority("cloud")).toBe("Cloud");
  });

  it("labels every AuthorityMode variant", () => {
    expect(formatAuthority("local")).toBe("Local");
    expect(formatAuthority("external")).toBe("External");
    expect(formatAuthority("immutable_import")).toBe("Immutable import");
  });
});

describe("formatMaterialization", () => {
  it("labels metadata_only after cloud upload", () => {
    expect(formatMaterialization("metadata_only")).toBe("Metadata only");
  });
});

describe("persistModeFromResourceStat", () => {
  const baseStat: ResourceStat = {
    resource_id: "018f3c2a-7b2e-7f3a-9c1d-2e4f5a6b7c8d",
    path: "Notes.md",
    authority: "local",
    materialization: "pinned",
    content_hash: null,
    version_id: null,
  };

  it("returns plain when Labs collab is off", () => {
    const stat: ResourceStat = {
      ...baseStat,
      resource_authority: {
        kind: "collaborative",
        doc_id: baseStat.resource_id,
      },
    };
    expect(
      persistModeFromResourceStat(stat, baseStat.resource_id, false),
    ).toBe("plain");
  });

  it("returns collaborative when authority doc_id matches registry id", () => {
    const stat: ResourceStat = {
      ...baseStat,
      resource_authority: {
        kind: "collaborative",
        doc_id: baseStat.resource_id,
      },
    };
    expect(
      persistModeFromResourceStat(stat, baseStat.resource_id, true),
    ).toBe("collaborative");
  });

  it("returns plain when collaborative doc_id does not match registry id", () => {
    const stat: ResourceStat = {
      ...baseStat,
      resource_authority: {
        kind: "collaborative",
        doc_id: "018f3c2a-7b2e-7f3a-9c1d-2e4f5a6b7c8e",
      },
    };
    expect(
      persistModeFromResourceStat(stat, baseStat.resource_id, true),
    ).toBe("plain");
  });
});

describe("persistModeFromResourceAuthority", () => {
  it("maps plain_file to plain", () => {
    expect(persistModeFromResourceAuthority({ kind: "plain_file" })).toBe("plain");
  });

  it("maps collaborative to collaborative", () => {
    expect(
      persistModeFromResourceAuthority({
        kind: "collaborative",
        doc_id: "018f3c2a-7b2e-7f3a-9c1d-2e4f5a6b7c8d",
      }),
    ).toBe("collaborative");
  });
});

describe("formatResourceAuthority", () => {
  it("uses toolbar labels for editing authority", () => {
    expect(formatResourceAuthority({ kind: "plain_file" })).toBe("Plain file");
    expect(
      formatResourceAuthority({
        kind: "collaborative",
        doc_id: "018f3c2a-7b2e-7f3a-9c1d-2e4f5a6b7c8d",
      }),
    ).toBe("Collaborative");
  });
});

describe("resourceAuthorityForPersistMode", () => {
  const registryId = "018f3c2a-7b2e-7f3a-9c1d-2e4f5a6b7c8d";

  it("maps plain to PlainFile authority", () => {
    expect(resourceAuthorityForPersistMode("plain", registryId)).toEqual({
      kind: "plain_file",
    });
  });

  it("maps collaborative with registry doc id and null revision", () => {
    expect(resourceAuthorityForPersistMode("collaborative", registryId)).toEqual({
      kind: "collaborative",
      doc_id: registryId,
      materialized_revision: null,
    });
  });
});
