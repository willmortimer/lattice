import { useMemo, useState } from "react";

import { cellValueToDisplay, type DataColumn, type DataRow } from "./types";
import {
  groupRowsByCalendarDate,
  resolveCalendarDateColumn,
  resolveListPrimaryColumn,
  resolveListSubtitleColumn,
} from "./viewLayout";

type CalendarMode = "month" | "week";

interface DataCalendarViewProps {
  rows: DataRow[];
  columns: DataColumn[];
  dateField?: string | null;
  selectedRowId?: string | null;
  onRowOpen: (row: DataRow) => void;
}

interface CalendarCell {
  isoDate: string;
  day: number;
  inMonth: boolean;
}

const WEEKDAY_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function toIsoDate(year: number, month: number, day: number): string {
  const paddedMonth = String(month + 1).padStart(2, "0");
  const paddedDay = String(day).padStart(2, "0");
  return `${year}-${paddedMonth}-${paddedDay}`;
}

function parseIsoDate(isoDate: string): { year: number; month: number; day: number } {
  const [year, month, day] = isoDate.split("-").map((part) => Number.parseInt(part, 10));
  return { year: year!, month: month! - 1, day: day! };
}

function startOfMonth(year: number, month: number): Date {
  return new Date(year, month, 1);
}

function daysInMonth(year: number, month: number): number {
  return new Date(year, month + 1, 0).getDate();
}

function buildMonthGrid(year: number, month: number): CalendarCell[] {
  const firstWeekday = startOfMonth(year, month).getDay();
  const totalDays = daysInMonth(year, month);
  const cells: CalendarCell[] = [];

  const prevMonth = month === 0 ? 11 : month - 1;
  const prevYear = month === 0 ? year - 1 : year;
  const prevMonthDays = daysInMonth(prevYear, prevMonth);
  for (let index = firstWeekday - 1; index >= 0; index -= 1) {
    const day = prevMonthDays - index;
    cells.push({
      isoDate: toIsoDate(prevYear, prevMonth, day),
      day,
      inMonth: false,
    });
  }

  for (let day = 1; day <= totalDays; day += 1) {
    cells.push({
      isoDate: toIsoDate(year, month, day),
      day,
      inMonth: true,
    });
  }

  const nextMonth = month === 11 ? 0 : month + 1;
  const nextYear = month === 11 ? year + 1 : year;
  let nextDay = 1;
  while (cells.length % 7 !== 0) {
    cells.push({
      isoDate: toIsoDate(nextYear, nextMonth, nextDay),
      day: nextDay,
      inMonth: false,
    });
    nextDay += 1;
  }

  return cells;
}

function buildWeekGrid(anchorIsoDate: string): CalendarCell[] {
  const { year, month, day } = parseIsoDate(anchorIsoDate);
  const anchor = new Date(year, month, day);
  const weekday = anchor.getDay();
  const cells: CalendarCell[] = [];
  for (let offset = 0; offset < 7; offset += 1) {
    const cellDate = new Date(year, month, day - weekday + offset);
    cells.push({
      isoDate: toIsoDate(cellDate.getFullYear(), cellDate.getMonth(), cellDate.getDate()),
      day: cellDate.getDate(),
      inMonth: cellDate.getMonth() === month,
    });
  }
  return cells;
}

function monthLabel(year: number, month: number): string {
  return new Date(year, month, 1).toLocaleString(undefined, {
    month: "long",
    year: "numeric",
  });
}

function weekLabel(cells: CalendarCell[]): string {
  const first = parseIsoDate(cells[0]!.isoDate);
  const last = parseIsoDate(cells[6]!.isoDate);
  const start = new Date(first.year, first.month, first.day);
  const end = new Date(last.year, last.month, last.day);
  const sameMonth = start.getMonth() === end.getMonth() && start.getFullYear() === end.getFullYear();
  if (sameMonth) {
    return `${start.toLocaleString(undefined, { month: "short" })} ${start.getDate()}–${end.getDate()}, ${end.getFullYear()}`;
  }
  return `${start.toLocaleDateString(undefined, { month: "short", day: "numeric" })} – ${end.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })}`;
}

function shiftMonth(year: number, month: number, delta: number): { year: number; month: number } {
  const next = new Date(year, month + delta, 1);
  return { year: next.getFullYear(), month: next.getMonth() };
}

function shiftWeek(isoDate: string, deltaWeeks: number): string {
  const { year, month, day } = parseIsoDate(isoDate);
  const next = new Date(year, month, day + deltaWeeks * 7);
  return toIsoDate(next.getFullYear(), next.getMonth(), next.getDate());
}

export function DataCalendarView({
  rows,
  columns,
  dateField,
  selectedRowId,
  onRowOpen,
}: DataCalendarViewProps) {
  const todayIso = useMemo(() => {
    const now = new Date();
    return toIsoDate(now.getFullYear(), now.getMonth(), now.getDate());
  }, []);
  const [mode, setMode] = useState<CalendarMode>("month");
  const [cursor, setCursor] = useState(() => {
    const now = new Date();
    return { year: now.getFullYear(), month: now.getMonth() };
  });
  const [weekAnchor, setWeekAnchor] = useState(todayIso);

  const dateColumn = useMemo(
    () => resolveCalendarDateColumn(columns, dateField),
    [columns, dateField],
  );
  const primaryColumn = useMemo(() => resolveListPrimaryColumn(columns), [columns]);
  const subtitleColumn = useMemo(
    () => resolveListSubtitleColumn(columns, primaryColumn),
    [columns, primaryColumn],
  );
  const buckets = useMemo(
    () => (dateColumn ? groupRowsByCalendarDate(rows, dateColumn) : []),
    [dateColumn, rows],
  );
  const rowsByDate = useMemo(() => {
    const map = new Map<string, DataRow[]>();
    for (const bucket of buckets) {
      map.set(bucket.key, bucket.rows);
    }
    return map;
  }, [buckets]);
  const undatedRows = rowsByDate.get("undated") ?? [];

  const monthCells = useMemo(
    () => buildMonthGrid(cursor.year, cursor.month),
    [cursor.month, cursor.year],
  );
  const weekCells = useMemo(() => buildWeekGrid(weekAnchor), [weekAnchor]);
  const activeCells = mode === "month" ? monthCells : weekCells;
  const periodLabel = mode === "month" ? monthLabel(cursor.year, cursor.month) : weekLabel(weekCells);

  if (!dateColumn) {
    return (
      <div className="data-calendar-empty">
        Add a <code>date</code> column or set <code>layout.date_field</code> in the view YAML to use
        calendar layout.
      </div>
    );
  }

  const handlePrev = () => {
    if (mode === "month") {
      setCursor((current) => shiftMonth(current.year, current.month, -1));
      return;
    }
    setWeekAnchor((current) => shiftWeek(current, -1));
  };

  const handleNext = () => {
    if (mode === "month") {
      setCursor((current) => shiftMonth(current.year, current.month, 1));
      return;
    }
    setWeekAnchor((current) => shiftWeek(current, 1));
  };

  const handleToday = () => {
    const now = new Date();
    setCursor({ year: now.getFullYear(), month: now.getMonth() });
    setWeekAnchor(todayIso);
  };

  return (
    <div className="data-calendar-view">
      <header className="data-calendar-toolbar">
        <div className="data-calendar-nav">
          <button type="button" className="secondary-button" onClick={handlePrev} aria-label="Previous">
            ←
          </button>
          <button type="button" className="secondary-button" onClick={handleToday}>
            Today
          </button>
          <button type="button" className="secondary-button" onClick={handleNext} aria-label="Next">
            →
          </button>
        </div>
        <h3 className="data-calendar-period">{periodLabel}</h3>
        <div className="data-calendar-mode" role="group" aria-label="Calendar range">
          <button
            type="button"
            className={`secondary-button${mode === "month" ? " data-calendar-mode--active" : ""}`}
            aria-pressed={mode === "month"}
            onClick={() => setMode("month")}
          >
            Month
          </button>
          <button
            type="button"
            className={`secondary-button${mode === "week" ? " data-calendar-mode--active" : ""}`}
            aria-pressed={mode === "week"}
            onClick={() => setMode("week")}
          >
            Week
          </button>
        </div>
      </header>

      <div className="data-calendar-grid" role="grid" aria-label={periodLabel}>
        {WEEKDAY_LABELS.map((label) => (
          <div key={label} className="data-calendar-weekday" role="columnheader">
            {label}
          </div>
        ))}
        {activeCells.map((cell) => {
          const dayRows = rowsByDate.get(cell.isoDate) ?? [];
          const isToday = cell.isoDate === todayIso;
          return (
            <div
              key={cell.isoDate}
              className={`data-calendar-day${cell.inMonth ? "" : " data-calendar-day--outside"}${
                isToday ? " data-calendar-day--today" : ""
              }`}
              role="gridcell"
            >
              <div className="data-calendar-day-head">
                <span className="data-calendar-day-number">{cell.day}</span>
                {dayRows.length > 0 && (
                  <span className="data-calendar-day-count">{dayRows.length}</span>
                )}
              </div>
              <div className="data-calendar-events">
                {dayRows.map((row) => {
                  const primary = primaryColumn
                    ? cellValueToDisplay(row.values[primaryColumn])
                    : row.id;
                  const subtitle = subtitleColumn
                    ? cellValueToDisplay(row.values[subtitleColumn])
                    : "";
                  const selected = selectedRowId === row.id;
                  const rawDate = cellValueToDisplay(row.values[dateColumn]);
                  const timeSuffix = rawDate.includes("T") ? ` · ${rawDate.slice(11, 16)}` : "";

                  return (
                    <button
                      key={row.id}
                      type="button"
                      className={`data-calendar-event${selected ? " data-calendar-event--selected" : ""}`}
                      onClick={() => onRowOpen(row)}
                      aria-current={selected ? "true" : undefined}
                    >
                      <span className="data-calendar-event-primary">
                        {primary || row.id}
                        {timeSuffix}
                      </span>
                      {subtitle && <span className="data-calendar-event-subtitle">{subtitle}</span>}
                    </button>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>

      {undatedRows.length > 0 && (
        <section className="data-calendar-undated" aria-label="Undated records">
          <header className="data-calendar-undated-head">
            <h4 className="data-calendar-undated-title">Undated</h4>
            <span className="data-calendar-undated-count">{undatedRows.length}</span>
          </header>
          <div className="data-calendar-undated-list">
            {undatedRows.map((row) => {
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
                  className={`data-calendar-event${selected ? " data-calendar-event--selected" : ""}`}
                  onClick={() => onRowOpen(row)}
                  aria-current={selected ? "true" : undefined}
                >
                  <span className="data-calendar-event-primary">{primary || row.id}</span>
                  {subtitle && <span className="data-calendar-event-subtitle">{subtitle}</span>}
                </button>
              );
            })}
          </div>
        </section>
      )}
    </div>
  );
}
