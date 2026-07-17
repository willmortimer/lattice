import { useEffect, useMemo, useRef, useState } from "react";
import { CanvasOutline } from "./CanvasOutline";
import { CanvasStaleRevisionError, keyboardMoveDelta, previewPlaceResource, type CanvasAdapter } from "./adapter";
import { CanvasParseError, parseCanvas, type CanvasData } from "./types";
import { CanvasScene } from "./scene";

interface CanvasViewerProps {
  json: unknown;
  onOpenFile: (path: string) => void;
  adapter?: CanvasAdapter;
  baseRevision: string;
  onRevisionChange?: (revision: string) => void;
  onError?: (message: string) => void;
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

/** Pixi owns the scene hot loop; the DOM outline remains the accessible action surface. */
export function CanvasViewer({ json, onOpenFile, adapter, baseRevision, onRevisionChange, onError }: CanvasViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const onOpenFileRef = useRef(onOpenFile);
  const adapterRef = useRef(adapter);
  const revisionRef = useRef(baseRevision);
  const sceneRef = useRef<CanvasScene | null>(null);
  onOpenFileRef.current = onOpenFile;
  adapterRef.current = adapter;
  revisionRef.current = baseRevision;

  const parsed = useMemo(() => parse(json), [json]);
  const [data, setData] = useState<CanvasData | null>(parsed.data);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const reportError = (message: string) => {
    setErrorMessage(message);
    onError?.(message);
  };

  useEffect(() => {
    setData(parsed.data);
    setSelectedId(null);
  }, [parsed.data]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !data) return;

    let cancelled = false;
    const scene = new CanvasScene(host, {
      onOpenFile: (path) => onOpenFileRef.current(path),
      onSelectNode: setSelectedId,
      onMoveNodes: (nodes) => {
        const currentAdapter = adapterRef.current;
        if (!currentAdapter) return;
        void currentAdapter.moveNodes(nodes, revisionRef.current).then((revision) => {
          revisionRef.current = revision;
          setErrorMessage(null);
          onRevisionChange?.(revision);
        }).catch((error: unknown) => {
          reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error));
        });
      },
      onRemoveNodes: (nodeIds) => {
        const currentAdapter = adapterRef.current;
        if (!currentAdapter) return;
        void currentAdapter.removeNodes(nodeIds, revisionRef.current).then((revision) => {
          revisionRef.current = revision;
          setErrorMessage(null);
          onRevisionChange?.(revision);
          setData((current) => current ? {
            nodes: current.nodes.filter((node) => !nodeIds.includes(node.id)),
            edges: current.edges.filter((edge) => !nodeIds.includes(edge.fromNode) && !nodeIds.includes(edge.toNode)),
          } : current);
        }).catch((error: unknown) => {
          reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error));
        });
      },
    });
    sceneRef.current = scene;

    scene.ready.then(() => {
      if (!cancelled) scene.setData(data);
    });
    return () => {
      cancelled = true;
      sceneRef.current = null;
      scene.destroy();
    };
  }, [data, onError, onRevisionChange]);

  if (parsed.error) {
    return (
      <div className="placeholder">
        <p className="placeholder-copy">Couldn't parse this canvas.</p>
        <p className="placeholder-sub"><code>{parsed.error}</code></p>
      </div>
    );
  }
  if (!data) return null;

  const removeFromOutline = (id: string) => {
    const currentAdapter = adapterRef.current;
    if (!currentAdapter) {
      setData((current) => current ? {
        nodes: current.nodes.filter((node) => node.id !== id),
        edges: current.edges.filter((edge) => edge.fromNode !== id && edge.toNode !== id),
      } : current);
      return;
    }
    void currentAdapter.removeNodes([id], revisionRef.current).then((revision) => {
      revisionRef.current = revision;
      setErrorMessage(null);
      onRevisionChange?.(revision);
      setData((current) => current ? {
        nodes: current.nodes.filter((node) => node.id !== id),
        edges: current.edges.filter((edge) => edge.fromNode !== id && edge.toNode !== id),
      } : current);
    }).catch((error: unknown) => reportError(String(error)));
  };

  const placeResource = () => {
    const resourcePath = window.prompt("Workspace resource path", "Notes/")?.trim();
    if (!resourcePath) return;
    const node = {
      id: `resource-${data.nodes.length + 1}`,
      x: 80 + (data.nodes.length % 5) * 40,
      y: 80 + Math.floor(data.nodes.length / 5) * 40,
      width: 320,
      height: 200,
    };
    const currentAdapter = adapterRef.current;
    if (!currentAdapter) {
      setData((current) => current ? previewPlaceResource(current, resourcePath, node) : current);
      return;
    }
    void currentAdapter.placeResource({
      resourcePath,
      nodeId: node.id,
      x: node.x,
      y: node.y,
      width: node.width,
      height: node.height,
      baseRevision: revisionRef.current,
    }).then((revision) => {
      revisionRef.current = revision;
      setErrorMessage(null);
      onRevisionChange?.(revision);
      setData((current) => current ? previewPlaceResource(current, resourcePath, node) : current);
    }).catch((error: unknown) => reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error)));
  };

  return (
    <div
      className="canvas-surface"
      tabIndex={0}
      onKeyDown={(event) => {
        const delta = keyboardMoveDelta(event.key, event.shiftKey);
        if (delta && sceneRef.current?.moveSelectedBy(delta.x, delta.y)) event.preventDefault();
        if ((event.key === "Delete" || event.key === "Backspace") && sceneRef.current?.removeSelected()) {
          event.preventDefault();
        }
      }}
    >
      <div className="canvas-toolbar" aria-label="Canvas editing actions">
        <button type="button" onClick={placeResource}>Insert Resource</button>
        <button type="button" onClick={placeResource}>Place</button>
        <button type="button" onClick={() => sceneRef.current?.moveSelectedBy(10, 0)}>Move</button>
        <button type="button" onClick={() => sceneRef.current?.removeSelected()}>Remove</button>
      </div>
      {errorMessage && <p className="canvas-conflict" role="alert">{errorMessage}</p>}
      <div ref={hostRef} className="canvas-viewer" />
      <CanvasOutline
        nodes={data.nodes}
        selectedId={selectedId}
        onSelect={(id) => {
          setSelectedId(id);
          sceneRef.current?.selectNode(id);
        }}
        onRemove={removeFromOutline}
      />
    </div>
  );
}
