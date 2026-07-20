import { describe, expect, it } from "vitest";

import type { DataColumn, DataRow } from "./types";
import {
  buildRelationLabelIndex,
  extractRelationIds,
  findInboundRelationLinks,
  formatCellForColumnName,
  formatColumnCellDisplay,
  formatRelationDisplay,
  parseRelationDraft,
  relationCellValue,
  relationDraftFromIds,
  relationIdsEqual,
  relationRecordLabel,
  syncRelationTargetsAfterDelete,
  syncRelationTargetsAfterUpsert,
} from "./relationDisplay";
import type { DataAppSnapshot } from "./types";

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

const contactColumns: DataColumn[] = [
  { name: "id", field_type: "text", sqlite_type: "TEXT" },
  { name: "name", field_type: "text", sqlite_type: "TEXT" },
  {
    name: "reports_to",
    field_type: "relation",
    sqlite_type: "TEXT",
    relation_table: "contacts",
  },
];

const contactRows: DataRow[] = [
  {
    id: "c_ada",
    values: {
      id: { Text: "c_ada" },
      name: { Text: "Ada Lovelace" },
      reports_to: { Null: null },
    },
  },
  {
    id: "c_grace",
    values: {
      id: { Text: "c_grace" },
      name: { Text: "Grace Hopper" },
      reports_to: { Relation: { record_ids: ["c_ada"] } },
    },
  },
  {
    id: "c_alan",
    values: {
      id: { Text: "c_alan" },
      name: { Text: "Alan Turing" },
      reports_to: { Relation: { record_ids: ["c_ada"] } },
    },
  },
];

describe("relationDisplay helpers", () => {
  it("extracts relation ids and builds cell values", () => {
    expect(extractRelationIds({ Null: null })).toEqual([]);
    expect(extractRelationIds("Null")).toEqual([]);
    expect(extractRelationIds({ Relation: { record_ids: ["a", "b"] } })).toEqual(["a", "b"]);
    expect(extractRelationIds({ Relation: {} as { record_ids: string[] } })).toEqual([]);
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

  it("formats relation columns via shared column helpers", () => {
    const index = buildRelationLabelIndex({ companies: companyRows });
    const relationColumn = {
      name: "company",
      field_type: "relation" as const,
      sqlite_type: "TEXT",
      relation_table: "companies",
    };
    const textColumn = {
      name: "name",
      field_type: "text" as const,
      sqlite_type: "TEXT",
    };
    const relationValue = { Relation: { record_ids: ["co_1"] } };
    expect(formatColumnCellDisplay(relationValue, relationColumn, index)).toBe(
      "Analytical Engines",
    );
    expect(formatColumnCellDisplay({ Text: "Ada" }, textColumn, index)).toBe("Ada");
    expect(
      formatCellForColumnName(
        {
          id: "row_1",
          values: { company: relationValue },
        },
        "company",
        [relationColumn],
        index,
      ),
    ).toBe("Analytical Engines");
  });

  it("finds inbound relation links on the active table and in relation_targets", () => {
    const inbound = findInboundRelationLinks(
      "c_ada",
      "contacts",
      contactColumns,
      contactRows,
      {
        contacts: contactRows,
        projects: [
          {
            id: "p_1",
            values: {
              id: { Text: "p_1" },
              title: { Text: "Compiler audit" },
              owner: { Relation: { record_ids: ["c_ada"] } },
            },
          },
        ],
      },
    );

    expect(inbound).toEqual([
      {
        table: "contacts",
        column: "reports_to",
        sourceRow: contactRows[1],
        label: "Grace Hopper",
      },
      {
        table: "contacts",
        column: "reports_to",
        sourceRow: contactRows[2],
        label: "Alan Turing",
      },
      {
        table: "projects",
        column: "owner",
        sourceRow: {
          id: "p_1",
          values: {
            id: { Text: "p_1" },
            title: { Text: "Compiler audit" },
            owner: { Relation: { record_ids: ["c_ada"] } },
          },
        },
        label: "Compiler audit",
      },
    ]);
  });

  it("prefers active-table rows over relation_targets for self-relations", () => {
    const staleTargets = {
      contacts: [
        {
          id: "c_grace",
          values: {
            id: { Text: "c_grace" },
            name: { Text: "Stale Grace" },
            reports_to: { Relation: { record_ids: ["c_ada"] } },
          },
        },
      ],
    };

    const inbound = findInboundRelationLinks(
      "c_ada",
      "contacts",
      contactColumns,
      contactRows,
      staleTargets,
    );

    expect(inbound).toHaveLength(2);
    expect(inbound.every((link) => link.label !== "Stale Grace")).toBe(true);
    expect(inbound.map((link) => link.sourceRow.id).sort()).toEqual(["c_alan", "c_grace"]);
  });

  it("returns no inbound links when nothing points at the row", () => {
    expect(
      findInboundRelationLinks("c_alan", "contacts", contactColumns, contactRows),
    ).toEqual([]);
  });

  it("keeps relation_targets in sync after row upsert and delete", () => {
    const snapshot: DataAppSnapshot = {
      title: "CRM",
      default_table: "contacts",
      package_revision: "rev:0",
      columns: [
        { name: "id", field_type: "text", sqlite_type: "TEXT" },
        { name: "name", field_type: "text", sqlite_type: "TEXT" },
        {
          name: "reports_to",
          field_type: "relation",
          sqlite_type: "TEXT",
          relation_table: "contacts",
        },
      ],
      rows: [
        {
          id: "c_1",
          values: {
            id: { Text: "c_1" },
            name: { Text: "Ada" },
            reports_to: { Null: null },
          },
        },
      ],
      row_offset: 0,
      row_limit: 1,
      row_total: 1,
      has_more: false,
      available_views: ["All"],
      active_view: "All",
      filters: [],
      layout_type: "grid",
      relation_targets: {
        contacts: [
          {
            id: "c_1",
            values: {
              id: { Text: "c_1" },
              name: { Text: "Ada" },
              reports_to: { Null: null },
            },
          },
        ],
      },
    };
    const updatedRow = {
      id: "c_1",
      values: {
        id: { Text: "c_1" },
        name: { Text: "Augusta Ada King" },
        reports_to: { Null: null },
      },
    };
    const afterUpsert = syncRelationTargetsAfterUpsert(snapshot, updatedRow);
    expect(afterUpsert?.contacts?.[0]?.values.name).toEqual({ Text: "Augusta Ada King" });

    const afterDelete = syncRelationTargetsAfterDelete(
      { ...snapshot, relation_targets: afterUpsert },
      "c_1",
    );
    expect(afterDelete?.contacts).toBeUndefined();
  });
});
