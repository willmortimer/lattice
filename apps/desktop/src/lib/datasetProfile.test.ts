import { describe, expect, it } from "vitest";
import { formatDistinct, formatPercent, formatProfileSummary } from "./datasetProfile";
import type { RelationProfile } from "./datasetProfile";

describe("datasetProfile formatters", () => {
  it("formats null percentage and distinct counts", () => {
    expect(formatPercent(12.345)).toBe("12.3%");
    expect(formatPercent(undefined)).toBe("—");
    expect(formatDistinct(1200)).toBe("~1,200");
    expect(formatDistinct(undefined)).toBe("—");
  });

  it("summarizes row and column counts", () => {
    const profile: RelationProfile = {
      rowCount: 1500,
      relationSql: "SELECT 1",
      columns: [
        { name: "id", dataType: "BIGINT" },
        { name: "name", dataType: "VARCHAR" },
      ],
    };
    expect(formatProfileSummary(profile)).toBe("1,500 rows · 2 columns");
  });
});
