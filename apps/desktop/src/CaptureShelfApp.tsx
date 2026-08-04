import { emitTo, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface CaptureShelfEntry {
  pagePath: string;
  title: string;
  ingestedAtMs: number;
}

interface CaptureShelfSnapshot {
  entries: CaptureShelfEntry[];
  count: number;
  latestTitle: string | null;
  destinationDirectory: string | null;
  workspaceRoot: string | null;
  workspaceName: string | null;
}

function formatTimestamp(ingestedAtMs: number): string {
  return new Date(ingestedAtMs).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

function savedDestinationLabel(snapshot: CaptureShelfSnapshot | null, count: number): string | null {
  if (!snapshot || count === 0 || !snapshot.destinationDirectory) return null;
  const workspace = snapshot.workspaceName ?? "workspace";
  return `Saved to ${workspace} / ${snapshot.destinationDirectory}`;
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
  const destinationLabel = savedDestinationLabel(snapshot, count);

  async function hideShelf() {
    await invoke("capture_shelf_hide");
  }

  async function openClip(entry: CaptureShelfEntry) {
    const root = snapshot?.workspaceRoot;
    if (!root) return;
    await emitTo("main", "open-resource", { root, path: entry.pagePath });
    await hideShelf();
  }

  return (
    <div className="capture-shelf" data-testid="capture-shelf">
      <header className="capture-shelf__header" data-tauri-drag-region>
        <div className="capture-shelf__heading">
          <h1 className="capture-shelf__title">Capture Shelf</h1>
          {destinationLabel ? (
            <p className="capture-shelf__destination">{destinationLabel}</p>
          ) : null}
        </div>
        <div className="capture-shelf__header-actions">
          <span className="capture-shelf__count">{count} clip{count === 1 ? "" : "s"}</span>
          <button
            type="button"
            className="capture-shelf__hide"
            aria-label="Hide capture shelf"
            onClick={() => void hideShelf()}
          >
            ×
          </button>
        </div>
      </header>
      {count === 0 ? (
        <p className="capture-shelf__empty">Recent screen clips appear here.</p>
      ) : (
        <ul className="capture-shelf__list">
          {snapshot?.entries.map((entry) => (
            <li key={`${entry.pagePath}-${entry.ingestedAtMs}`}>
              <button
                type="button"
                className="capture-shelf__item"
                onClick={() => void openClip(entry)}
              >
                <p className="capture-shelf__item-title">{entry.title}</p>
                <p className="capture-shelf__item-meta">{formatTimestamp(entry.ingestedAtMs)}</p>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
