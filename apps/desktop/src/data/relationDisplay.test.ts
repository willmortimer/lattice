import { describe, expect, it } from "vitest";

import type { DataRow } from "./types";
import {
  buildRelationLabelIndex,
  extractRelationIds,
  formatRelationDisplay,
  parseRelationDraft,
  relationCellValue,
  relationDraftFromIds,
  relationIdsEqual,
  relationRecordLabel,
} from "./relationDisplay";

const companyRows: DataRow[] = [
  {
    id: "co_1",
    values: {
      id: { Text: "co_1" },
      name: { Text: "Analytical Engines" },
    },
  },
  {
    id: "co_2",
    values: {
      id: { Text: "co_2" },
      name: { Text: "US Navy" },
    },
  },
];

describe("relationDisplay helpers", () => {
  it("extracts relation ids and builds cell values", () => {
    expect(extractRelationIds({ Null: null })).toEqual([]);
    expect(extractRelationIds({ Relation: { record_ids: ["a", "b"] } })).toEqual(["a", "b"]);
    expect(relationCellValue([])).toEqual({ Null: null });
    expect(relationCellValue(["a"])).toEqual({ Relation: { record_ids: ["a"] } });
  });

  it("round-trips relation drafts as JSON id arrays", () => {
    const ids = ["co_1", "co_2"];
    expect(relationDraftFromIds(ids)).toBe('["co_1","co_2"]');
    expect(parseRelationDraft(relationDraftFromIds(ids))).toEqual(ids);
    expect(parseRelationDraft("")).toEqual([]);
    expect(parseRelationDraft("not-json")).toEqual([]);
    expect(parseRelationDraft('{"bad":"shape"}')).toEqual([]);
  });

  it("compares relation id lists in order", () => {
    expect(relationIdsEqual(["a", "b"], ["a", "b"])).toBe(true);
    expect(relationIdsEqual(["a"], ["b"])).toBe(false);
    expect(relationIdsEqual(["a", "b"], ["b", "a"])).toBe(false);
  });

  it("labels related rows from name-like fields", () => {
    expect(relationRecordLabel(companyRows[0]!)).toBe("Analytical Engines");
    expect(
      relationRecordLabel({
        id: "rec_x",
        values: { id: { Text: "rec_x" }, notes: { Text: "fallback" } },
      }),
    ).toBe("fallback");
    expect(
      relationRecordLabel({
        id: "rec_only",
        values: { id: { Text: "rec_only" } },
      }),
    ).toBe("rec_only");
  });

  it("formats linked titles when target rows are available", () => {
    const index = buildRelationLabelIndex({ companies: companyRows });
    expect(formatRelationDisplay(["co_1", "co_2"], "companies", index)).toBe(
      "Analytical Engines, US Navy",
    );
    expect(formatRelationDisplay(["missing"], "companies", index)).toBe("missing");
    expect(formatRelationDisplay([], "companies", index)).toBe("");
  });
});
