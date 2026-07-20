import { useEffect, useRef, useState } from "react";
import {
  ensurePerspectiveRuntime,
  ipcBytesToArrayBuffer,
  type PerspectiveTable,
  type PerspectiveViewerElement,
} from "./perspectiveRuntime";
import "./perspective.css";

export interface PerspectiveDatasetViewerProps {
  /** Arrow IPC stream bytes from `query_dataset_arrow`. */
  ipcBytes: Uint8Array | number[] | ArrayBuffer;
  /** Bump to force a reload (e.g. after re-query). */
  loadKey?: string | number;
  onReady?: () => void;
  onError?: (message: string) => void;
}

/**
 * Hosts `<perspective-viewer>` and feeds it Arrow IPC without expanding rows
 * into JavaScript objects. Failures surface via `onError` so the parent can
 * fall back to a schema dump.
 */
export function PerspectiveDatasetViewer({
  ipcBytes,
  loadKey = 0,
  onReady,
  onError,
}: PerspectiveDatasetViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const tableRef = useRef<PerspectiveTable | null>(null);
  const ipcBytesRef = useRef(ipcBytes);
  const onReadyRef = useRef(onReady);
  const onErrorRef = useRef(onError);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");

  ipcBytesRef.current = ipcBytes;
  onReadyRef.current = onReady;
  onErrorRef.current = onError;

  useEffect(() => {
    let cancelled = false;
    const host = hostRef.current;
    if (!host) return;

    setStatus("loading");

    void (async () => {
      try {
        const runtime = await ensurePerspectiveRuntime();
        if (cancelled) return;

        host.replaceChildren();
        const viewer = document.createElement(
          "perspective-viewer",
        ) as PerspectiveViewerElement;
        viewer.className = "perspective-dataset-viewer-el";
        viewer.setAttribute("theme", "Pro Dark");
        host.append(viewer);

        const buffer = ipcBytesToArrayBuffer(ipcBytesRef.current);
        const tableOrPromise = runtime.worker.table(buffer);
        const table = await Promise.resolve(tableOrPromise);
        if (cancelled) {
          await Promise.resolve(table.delete());
          return;
        }
        tableRef.current = table;
        await viewer.load(table);
        if (cancelled) return;
        setStatus("ready");
        onReadyRef.current?.();
      } catch (err: unknown) {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : String(err);
        setStatus("error");
        onErrorRef.current?.(message);
      }
    })();

    return () => {
      cancelled = true;
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

  return (
    <div className="perspective-dataset-viewer" data-status={status}>
      {status === "loading" ? (
        <p className="perspective-dataset-viewer-status" aria-live="polite">
          Loading analytical grid…
        </p>
      ) : null}
      <div ref={hostRef} className="perspective-dataset-viewer-host" />
    </div>
  );
}
