import { useEffect, useMemo, useRef } from "react";
import { CanvasParseError, parseCanvas, type CanvasData } from "./types";
import { CanvasScene } from "./scene";

interface CanvasViewerProps {
  json: unknown;
  onOpenFile: (path: string) => void;
}

interface ParseResult {
  data: CanvasData | null;
  error: string | null;
}

function parse(json: unknown): ParseResult {
  try {
    return { data: parseCanvas(json), error: null };
  } catch (err) {
    return { data: null, error: err instanceof CanvasParseError ? err.message : String(err) };
  }
}

/** Read-only JSON Canvas viewer: parses `json`, renders it with PixiJS. */
export function CanvasViewer({ json, onOpenFile }: CanvasViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const onOpenFileRef = useRef(onOpenFile);
  onOpenFileRef.current = onOpenFile;

  const { data, error } = useMemo(() => parse(json), [json]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !data) return;

    let cancelled = false;
    const scene = new CanvasScene(host, {
      onOpenFile: (path) => onOpenFileRef.current(path),
    });

    scene.ready.then(() => {
      if (!cancelled) scene.setData(data);
    });

    return () => {
      cancelled = true;
      scene.destroy();
    };
  }, [data]);

  if (error) {
    return (
      <div className="placeholder">
        <p className="placeholder-copy">Couldn't parse this canvas.</p>
        <p className="placeholder-sub">
          <code>{error}</code>
        </p>
      </div>
    );
  }

  return <div ref={hostRef} className="canvas-viewer" />;
}
