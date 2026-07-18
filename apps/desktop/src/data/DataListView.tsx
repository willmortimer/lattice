import { useMemo } from "react";

import type { RelationLabelIndex } from "./relationDisplay";
import { formatCellForColumnName } from "./relationDisplay";
import { type DataColumn, type DataRow } from "./types";
import {
  resolveListPrimaryColumn,
  resolveListSubtitleColumn,
} from "./viewLayout";

interface DataListViewProps {
  rows: DataRow[];
  columns: DataColumn[];
  relationLabelIndex: RelationLabelIndex;
  selectedRowId?: string | null;
  zebraRows: boolean;
  onRowOpen: (row: DataRow) => void;
}

export function DataListView({
  rows,
  columns,
  relationLabelIndex,
  selectedRowId,
  zebraRows,
  onRowOpen,
}: DataListViewProps) {
  const primaryColumn = useMemo(() => resolveListPrimaryColumn(columns), [columns]);
  const subtitleColumn = useMemo(
    () => resolveListSubtitleColumn(columns, primaryColumn),
    [columns, primaryColumn],
  );

  return (
    <div className="data-list-view" role="list">
      {rows.map((row, index) => {
        const primary = primaryColumn
          ? formatCellForColumnName(row, primaryColumn, columns, relationLabelIndex)
          : row.id;
        const subtitle = subtitleColumn
          ? formatCellForColumnName(row, subtitleColumn, columns, relationLabelIndex)
          : "";
        const selected = selectedRowId === row.id;

        return (
          <button
            key={row.id}
            type="button"
            role="listitem"
            className={`data-list-row${selected ? " data-list-row--selected" : ""}${
              zebraRows && index % 2 === 1 ? " data-list-row--zebra" : ""
            }`}
            onClick={() => onRowOpen(row)}
            aria-current={selected ? "true" : undefined}
          >
            <span className="data-list-primary">{primary || row.id}</span>
            {subtitle && <span className="data-list-subtitle">{subtitle}</span>}
          </button>
        );
      })}
    </div>
  );
}
