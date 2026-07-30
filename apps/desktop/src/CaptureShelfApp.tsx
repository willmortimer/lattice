import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface CaptureShelfEntry {
  pagePath: string;
  ingestedAtMs: number;
}

interface CaptureShelfSnapshot {
  entries: CaptureShelfEntry[];
  count: number;
  latestTitle: string | null;
}

function formatTimestamp(ingestedAtMs: number): string {
  return new Date(ingestedAtMs).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function CaptureShelfApp() {
  const [snapshot, setSnapshot] = useState<CaptureShelfSnapshot | null>(null);

  useEffect(() => {
    let cancelled = false;
    void invoke<CaptureShelfSnapshot>("capture_shelf_snapshot").then((next) => {
      if (!cancelled) setSnapshot(next);
    });
    const unlistenPromise = listen<CaptureShelfSnapshot>("capture-shelf-updated", (event) => {
      setSnapshot(event.payload);
    });
    return () => {
      cancelled = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const count = snapshot?.count ?? 0;

  return (
    <div className="capture-shelf" data-testid="capture-shelf">
      <header className="capture-shelf__header">
        <h1 className="capture-shelf__title">Capture Shelf</h1>
        <span className="capture-shelf__count">{count} clip{count === 1 ? "" : "s"}</span>
      </header>
      {count === 0 ? (
        <p className="capture-shelf__empty">Recent screen clips appear here.</p>
      ) : (
        <ul className="capture-shelf__list">
          {snapshot?.entries.map((entry) => (
            <li key={`${entry.pagePath}-${entry.ingestedAtMs}`} className="capture-shelf__item">
              <p className="capture-shelf__item-title">{entry.pagePath}</p>
              <p className="capture-shelf__item-meta">{formatTimestamp(entry.ingestedAtMs)}</p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
