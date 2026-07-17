import { useMemo } from "react";

import { cellValueToDisplay, type DataColumn, type DataRow } from "./types";
import {
  groupRowsByColumn,
  resolveGroupByColumn,
  resolveListPrimaryColumn,
  resolveListSubtitleColumn,
} from "./viewLayout";

interface DataBoardViewProps {
  rows: DataRow[];
  columns: DataColumn[];
  groupBy?: string | null;
  selectedRowId?: string | null;
  onRowOpen: (row: DataRow) => void;
}

export function DataBoardView({
  rows,
  columns,
  groupBy,
  selectedRowId,
  onRowOpen,
}: DataBoardViewProps) {
  const groupColumn = useMemo(
    () => resolveGroupByColumn(columns, groupBy),
    [columns, groupBy],
  );
  const primaryColumn = useMemo(() => resolveListPrimaryColumn(columns), [columns]);
  const subtitleColumn = useMemo(
    () => resolveListSubtitleColumn(columns, primaryColumn),
    [columns, primaryColumn],
  );
  const lanes = useMemo(
    () => (groupColumn ? groupRowsByColumn(rows, groupColumn) : []),
    [groupColumn, rows],
  );

  if (!groupColumn) {
    return (
      <div className="data-board-empty">
        Add a text or boolean column (for example <code>status</code>) or set{" "}
        <code>layout.group_by</code> in the view YAML to use board layout.
      </div>
    );
  }

  return (
    <div className="data-board-view" role="list">
      {lanes.map((lane) => (
        <section key={lane.key} className="data-board-lane" aria-label={lane.key}>
          <header className="data-board-lane-head">
            <h3 className="data-board-lane-title">{lane.key}</h3>
            <span className="data-board-lane-count">{lane.rows.length}</span>
          </header>
          <div className="data-board-cards">
            {lane.rows.map((row) => {
              const primary = primaryColumn
                ? cellValueToDisplay(row.values[primaryColumn])
                : row.id;
              const subtitle = subtitleColumn
                ? cellValueToDisplay(row.values[subtitleColumn])
                : "";
              const selected = selectedRowId === row.id;

              return (
                <button
                  key={row.id}
                  type="button"
                  className={`data-board-card${selected ? " data-board-card--selected" : ""}`}
                  onClick={() => onRowOpen(row)}
                  aria-current={selected ? "true" : undefined}
                >
                  <span className="data-board-card-primary">{primary || row.id}</span>
                  {subtitle && <span className="data-board-card-subtitle">{subtitle}</span>}
                </button>
              );
            })}
          </div>
        </section>
      ))}
    </div>
  );
}
