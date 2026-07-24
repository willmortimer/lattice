import { useEffect, useRef, useState } from "react";
import {
  ensurePerspectiveRuntime,
  ipcBytesToArrayBuffer,
  type PerspectiveTable,
  type PerspectiveViewerElement,
} from "./perspectiveRuntime";
import type { ArrowFieldMeta } from "../lib/arrowIpc";
import { arrowIpcToValues, sampleRowsToValues } from "../lib/arrowToVegaData";
import "./perspective.css";

/** Hard cap on rows decoded from Arrow IPC into JSON for the grid. */
const MAX_PREVIEW_ROWS = 10_000;

export interface PerspectiveDatasetViewerProps {
  /** Arrow IPC stream bytes from `query_dataset_arrow`. */
  ipcBytes: Uint8Array | number[] | ArrayBuffer;
  /** Control-plane schema (used for JSON load + diagnostics). */
  schema?: ArrowFieldMeta[];
  /** Bounded JSON preview rows from the query control message. */
  sampleRows?: unknown[][];
  /** Declared row count from the Arrow transport control message. */
  rowCount?: number;
  /** Whether the server truncated the result at its row/byte limit. */
  truncated?: boolean;
  /** Bump to force a reload (e.g. after re-query). */
  loadKey?: string | number;
  /** Show the diagnostics panel (Settings → Diagnostics → renderer stats). */
  showDiagnostics?: boolean;
  onReady?: () => void;
  onError?: (message: string) => void;
}

type LoadPath = "json-arrow-decode" | "json-sample";

export type PerspectiveDebugInfo = {
  loadPath: LoadPath;
  ipcBytes: number;
  expectedRows: number;
  loadedRows: number;
  tableSize: number | null;
  hostWidth: number;
  hostHeight: number;
  viewerWidth: number;
  viewerHeight: number;
  note: string;
};

/**
 * Hosts `<perspective-viewer>` for dataset Preview.
 *
 * Loads JSON row objects decoded from the Arrow IPC payload via apache-arrow
 * (bounded at {@link MAX_PREVIEW_ROWS}) — Perspective's native Arrow ingestion
 * has painted schema chrome with an empty Datagrid body under Tauri WKWebView,
 * so the JSON path is deliberate. The control-message sample is only a last
 * resort when IPC decode yields nothing.
 */
export function PerspectiveDatasetViewer({
  ipcBytes,
  schema = [],
  sampleRows = [],
  rowCount = 0,
  truncated = false,
  loadKey = 0,
  showDiagnostics = false,
  onReady,
  onError,
}: PerspectiveDatasetViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const tableRef = useRef<PerspectiveTable | null>(null);
  const ipcBytesRef = useRef(ipcBytes);
  const schemaRef = useRef(schema);
  const sampleRowsRef = useRef(sampleRows);
  const rowCountRef = useRef(rowCount);
  const onReadyRef = useRef(onReady);
  const onErrorRef = useRef(onError);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [loadedRows, setLoadedRows] = useState(0);
  const [debug, setDebug] = useState<PerspectiveDebugInfo | null>(null);

  ipcBytesRef.current = ipcBytes;
  schemaRef.current = schema;
  sampleRowsRef.current = sampleRows;
  rowCountRef.current = rowCount;
  onReadyRef.current = onReady;
  onErrorRef.current = onError;

  useEffect(() => {
    let cancelled = false;
    const host = hostRef.current;
    if (!host) return;

    setStatus("loading");
    setLoadedRows(0);
    setDebug(null);

    void (async () => {
      try {
        const runtime = await ensurePerspectiveRuntime();
        if (cancelled) return;

        host.replaceChildren();
        const viewer = document.createElement(
          "perspective-viewer",
        ) as PerspectiveViewerElement;
        viewer.className = "perspective-dataset-viewer-el";
        viewer.style.display = "block";
        viewer.style.width = "100%";
        viewer.style.height = "100%";
        // Structural base theme; perspective.css re-colors it from --lt-* tokens.
        viewer.setAttribute("theme", "Pro Dark");
        // Explicit plugin — without it WKWebView sometimes paints an empty chrome.
        viewer.setAttribute("plugin", "Datagrid");
        host.append(viewer);

        const buffer = ipcBytesToArrayBuffer(ipcBytesRef.current);
        const { table, rows, loadPath, note } = await buildPerspectiveTable(
          runtime.worker,
          {
            buffer,
            schema: schemaRef.current,
            sampleRows: sampleRowsRef.current,
          },
        );
        if (cancelled) {
          await Promise.resolve(table.delete());
          return;
        }

        tableRef.current = table;
        setLoadedRows(rows);
        await viewer.load(table);
        try {
          await viewer.restore?.({
            plugin: "Datagrid",
            settings: false,
          });
        } catch {
          /* older perspective builds omit restore */
        }

        const notify = () => {
          void Promise.resolve(viewer.notifyResize?.(true)).catch(() => {
            /* optional API */
          });
        };
        notify();
        requestAnimationFrame(() => {
          notify();
          requestAnimationFrame(notify);
        });

        if (cancelled) return;

        const tableSize = await readTableSize(table);
        const hostRect = host.getBoundingClientRect();
        const viewerRect = viewer.getBoundingClientRect();
        setDebug({
          loadPath,
          ipcBytes: buffer.byteLength,
          expectedRows: rowCountRef.current,
          loadedRows: rows,
          tableSize,
          hostWidth: Math.round(hostRect.width),
          hostHeight: Math.round(hostRect.height),
          viewerWidth: Math.round(viewerRect.width),
          viewerHeight: Math.round(viewerRect.height),
          note,
        });

        setStatus("ready");
        onReadyRef.current?.();
      } catch (err: unknown) {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : String(err);
        setStatus("error");
        onErrorRef.current?.(message);
      }
    })();

    const resizeObserver = new ResizeObserver(() => {
      const viewer = host.querySelector("perspective-viewer") as PerspectiveViewerElement | null;
      void Promise.resolve(viewer?.notifyResize?.(true)).catch(() => {
        /* optional API */
      });
      setDebug((prev) => {
        if (!prev || !viewer) return prev;
        const hostRect = host.getBoundingClientRect();
        const viewerRect = viewer.getBoundingClientRect();
        return {
          ...prev,
          hostWidth: Math.round(hostRect.width),
          hostHeight: Math.round(hostRect.height),
          viewerWidth: Math.round(viewerRect.width),
          viewerHeight: Math.round(viewerRect.height),
        };
      });
    });
    resizeObserver.observe(host);

    return () => {
      cancelled = true;
      resizeObserver.disconnect();
      const table = tableRef.current;
      tableRef.current = null;
      if (table) {
        void Promise.resolve(table.delete()).catch(() => {
          /* best-effort cleanup */
        });
      }
      host.replaceChildren();
    };
  }, [loadKey]);

  const showSample =
    debug !== null && sampleRows.length > 0 && schema.length > 0;

  const footer =
    status === "ready" && loadedRows > 0
      ? truncated
        ? `${loadedRows.toLocaleString()} of ${Math.max(rowCount, loadedRows).toLocaleString()}+ rows (truncated at query limit)`
        : loadedRows < rowCount
          ? `${loadedRows.toLocaleString()} of ${rowCount.toLocaleString()} rows (preview cap)`
          : `${loadedRows.toLocaleString()} row${loadedRows === 1 ? "" : "s"}`
      : null;

  return (
    <div className="perspective-dataset-viewer" data-status={status}>
      {status === "loading" ? (
        <p className="perspective-dataset-viewer-status" aria-live="polite">
          Loading analytical grid…
        </p>
      ) : null}
      {showDiagnostics && debug ? (
        <details
          className="perspective-dataset-debug"
          open={(debug.tableSize ?? 0) === 0 || debug.hostHeight < 120}
        >
          <summary>Preview diagnostics</summary>
          <pre>
            {`path=${debug.loadPath}
ipcBytes=${debug.ipcBytes}
expectedRows=${debug.expectedRows}
loadedRows=${debug.loadedRows}
tableSize=${debug.tableSize ?? "n/a"}
host=${debug.hostWidth}×${debug.hostHeight}
viewer=${debug.viewerWidth}×${debug.viewerHeight}
${debug.note}`}
          </pre>
          {showSample ? (
            <div className="perspective-dataset-sample">
              <p className="perspective-dataset-sample-label">Control-message sample rows</p>
              <table>
                <thead>
                  <tr>
                    {schema.map((field) => (
                      <th key={field.name} scope="col">
                        {field.name}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {sampleRows.map((row, rowIndex) => (
                    <tr key={rowIndex}>
                      {schema.map((field, colIndex) => (
                        <td key={field.name}>{formatSampleCell(row[colIndex])}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
        </details>
      ) : null}
      <div ref={hostRef} className="perspective-dataset-viewer-host" />
      {footer ? <p className="perspective-dataset-footer">{footer}</p> : null}
    </div>
  );
}

async function buildPerspectiveTable(
  worker: {
    table: (
      data: ArrayBuffer | Record<string, unknown>[],
      options?: { name?: string; format?: string },
    ) => Promise<PerspectiveTable> | PerspectiveTable;
  },
  input: {
    buffer: ArrayBuffer;
    schema: ArrowFieldMeta[];
    sampleRows: unknown[][];
  },
): Promise<{ table: PerspectiveTable; rows: number; loadPath: LoadPath; note: string }> {
  // Full transported batch first: decode Arrow IPC with apache-arrow into plain
  // row objects and feed Perspective JSON. Native Arrow ingestion is avoided on
  // purpose (empty Datagrid body under WKWebView).
  if (input.buffer.byteLength > 0) {
    try {
      const decoded = arrowIpcToValues(input.buffer, MAX_PREVIEW_ROWS);
      if (decoded.length > 0) {
        const table = await Promise.resolve(worker.table(decoded));
        return {
          table,
          rows: decoded.length,
          loadPath: "json-arrow-decode",
          note: "Decoded Arrow IPC via apache-arrow, then JSON → Perspective (WKWebView-safe path)",
        };
      }
    } catch {
      /* fall back to the bounded control-message sample */
    }
  }

  const fromSample = sampleRowsToValues(input.sampleRows, input.schema);
  if (fromSample.length > 0) {
    const table = await Promise.resolve(worker.table(fromSample));
    return {
      table,
      rows: fromSample.length,
      loadPath: "json-sample",
      note: "Arrow IPC decode yielded no rows — loaded bounded control-message sample instead",
    };
  }

  throw new Error("Dataset query returned empty Arrow IPC (no rows to display).");
}

async function readTableSize(table: PerspectiveTable): Promise<number | null> {
  if (!table.size) return null;
  try {
    return await Promise.resolve(table.size());
  } catch {
    return null;
  }
}

function formatSampleCell(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return String(value);
  }
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
