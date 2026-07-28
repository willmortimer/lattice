import { describe, expect, it } from "vitest";
import { formatAuthority, formatMaterialization } from "./resourceStat";

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
