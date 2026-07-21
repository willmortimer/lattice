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

export interface PerspectiveDatasetViewerProps {
  /** Arrow IPC stream bytes from `query_dataset_arrow`. */
  ipcBytes: Uint8Array | number[] | ArrayBuffer;
  /** Control-plane schema (used for JSON load + diagnostics). */
  schema?: ArrowFieldMeta[];
  /** Bounded JSON preview rows from the query control message. */
  sampleRows?: unknown[][];
  /** Declared row count from the Arrow transport control message. */
  rowCount?: number;
  /** Bump to force a reload (e.g. after re-query). */
  loadKey?: string | number;
  onReady?: () => void;
  onError?: (message: string) => void;
}

type LoadPath = "json-sample" | "json-arrow-decode" | "arrow-native";

export type PerspectiveDebugInfo = {
  loadPath: LoadPath;
  ipcBytes: number;
  expectedRows: number;
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
 * Bounded Preview prefers JSON row objects (control sample or apache-arrow
 * decode) because Perspective's native Arrow path has painted schema chrome
 * with an empty Datagrid body under Tauri WKWebView. Diagnostics stay visible
 * so we can verify table size vs host geometry.
 */
export function PerspectiveDatasetViewer({
  ipcBytes,
  schema = [],
  sampleRows = [],
  rowCount = 0,
  loadKey = 0,
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
        viewer.setAttribute("theme", "Pro Dark");
        // Explicit plugin — without it WKWebView sometimes paints an empty chrome.
        viewer.setAttribute("plugin", "Datagrid");
        host.append(viewer);

        const buffer = ipcBytesToArrayBuffer(ipcBytesRef.current);
        const { table, loadPath, note } = await buildPerspectiveTable(runtime.worker, {
          buffer,
          schema: schemaRef.current,
          sampleRows: sampleRowsRef.current,
        });
        if (cancelled) {
          await Promise.resolve(table.delete());
          return;
        }

        tableRef.current = table;
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

  return (
    <div className="perspective-dataset-viewer" data-status={status}>
      {status === "loading" ? (
        <p className="perspective-dataset-viewer-status" aria-live="polite">
          Loading analytical grid…
        </p>
      ) : null}
      {debug ? (
        <details
          className="perspective-dataset-debug"
          open={
            debug.loadPath === "arrow-native" ||
            (debug.tableSize ?? 0) === 0 ||
            debug.hostHeight < 120
          }
        >
          <summary>Preview diagnostics</summary>
          <pre>
            {`path=${debug.loadPath}
ipcBytes=${debug.ipcBytes}
expectedRows=${debug.expectedRows}
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
): Promise<{ table: PerspectiveTable; loadPath: LoadPath; note: string }> {
  const fromSample = sampleRowsToValues(input.sampleRows, input.schema);
  if (fromSample.length > 0) {
    const table = await Promise.resolve(worker.table(fromSample));
    return {
      table,
      loadPath: "json-sample",
      note: "Loaded control-message sample rows as JSON (WKWebView-safe Preview path)",
    };
  }

  if (input.buffer.byteLength > 0) {
    try {
      const decoded = arrowIpcToValues(input.buffer);
      if (decoded.length > 0) {
        const table = await Promise.resolve(worker.table(decoded));
        return {
          table,
          loadPath: "json-arrow-decode",
          note: "Decoded Arrow IPC via apache-arrow, then JSON → Perspective",
        };
      }
    } catch {
      /* try native Arrow next */
    }

    const table = await Promise.resolve(
      worker.table(input.buffer, { format: "arrow" }),
    );
    return {
      table,
      loadPath: "arrow-native",
      note: "Perspective native Arrow IPC loader (no JSON sample available)",
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
