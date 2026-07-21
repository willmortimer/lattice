import { useEffect, useMemo, useRef, useState } from "react";
import { CanvasOutline } from "./CanvasOutline";
import {
  CanvasStaleRevisionError,
  canvasRelativePath,
  keyboardMoveDelta,
  previewAddEdge,
  previewAddTextNode,
  previewMoveNodes,
  previewPlaceResource,
  previewRemoveEdges,
  previewResizeNodes,
  previewUpdateTextNode,
  type CanvasAdapter,
} from "./adapter";
import { CanvasParseError, parseCanvas, type CanvasData } from "./types";
import { CanvasScene } from "./scene";
import { LATTICE_RESOURCE_MIME, readResourceDragPayload } from "../lib/resourceDrag";
import type { Resource } from "../types";

const OUTLINE_OPEN_KEY = "lattice.canvas.outlineOpen";
const DEFAULT_NOTE_WIDTH = 200;
const DEFAULT_NOTE_HEIGHT = 140;

interface CanvasViewerProps {
  json: unknown;
  canvasPath: string;
  resources?: readonly Resource[];
  onOpenFile: (path: string, subpath?: string) => void;
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

function readOutlineOpen(): boolean {
  try {
    const raw = localStorage.getItem(OUTLINE_OPEN_KEY);
    if (raw === null) return true;
    return raw !== "0";
  } catch {
    return true;
  }
}

function fileLabel(path: string): string {
  return path.split("/").pop() ?? path;
}

/** Pixi owns the scene hot loop; the DOM outline remains the accessible action surface. */
export function CanvasViewer({
  json,
  canvasPath,
  resources = [],
  onOpenFile,
  adapter,
  baseRevision,
  onRevisionChange,
  onError,
}: CanvasViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const onOpenFileRef = useRef(onOpenFile);
  const adapterRef = useRef(adapter);
  const revisionRef = useRef(baseRevision);
  const onRevisionChangeRef = useRef(onRevisionChange);
  const onErrorRef = useRef(onError);
  const sceneRef = useRef<CanvasScene | null>(null);
  const fitNextLoadRef = useRef(true);
  const connectModeRef = useRef(false);
  const connectFromIdRef = useRef<string | null>(null);
  onOpenFileRef.current = onOpenFile;
  adapterRef.current = adapter;
  onRevisionChangeRef.current = onRevisionChange;
  onErrorRef.current = onError;

  const parsed = useMemo(() => parse(json), [json]);
  const [data, setData] = useState<CanvasData | null>(parsed.data);
  const dataRef = useRef<CanvasData | null>(parsed.data);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [outlineOpen, setOutlineOpen] = useState(readOutlineOpen);
  const [placeOpen, setPlaceOpen] = useState(false);
  const [placeQuery, setPlaceQuery] = useState("");
  const [connectMode, setConnectMode] = useState(false);
  const [connectFromId, setConnectFromId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [textEdit, setTextEdit] = useState<{ id: string; text: string } | null>(null);

  connectModeRef.current = connectMode;
  connectFromIdRef.current = connectFromId;
  dataRef.current = data;

  useEffect(() => {
    revisionRef.current = baseRevision;
  }, [baseRevision]);

  const reportError = (message: string) => {
    setErrorMessage(message);
    onErrorRef.current?.(message);
  };

  const commitRevision = (revision: string) => {
    revisionRef.current = revision;
    setErrorMessage(null);
    onRevisionChangeRef.current?.(revision);
  };

  const setOutlineOpenPersisted = (open: boolean) => {
    setOutlineOpen(open);
    try {
      localStorage.setItem(OUTLINE_OPEN_KEY, open ? "1" : "0");
    } catch {
      // private mode / quota — in-memory toggle still works
    }
  };

  const placeableResources = useMemo(() => {
    const query = placeQuery.trim().toLowerCase();
    return resources
      .filter((resource) => resource.kind !== "folder")
      .filter((resource) => resource.path !== canvasPath)
      .filter((resource) => !query || resource.path.toLowerCase().includes(query))
      .slice(0, 40);
  }, [resources, canvasPath, placeQuery]);

  useEffect(() => {
    fitNextLoadRef.current = true;
    setData(parsed.data);
    setSelectedId(null);
    setSelectedEdgeId(null);
    setConnectFromId(null);
    setTextEdit(null);
  }, [parsed.data]);

  const dropWorldPoint = (clientX: number, clientY: number) => {
    const world = sceneRef.current?.clientToWorld(clientX, clientY);
    if (world) return { x: world.x - 40, y: world.y - 40 };
    return { x: 120, y: 120 };
  };
  const connectNodes = (
    fromNode: string,
    toNode: string,
    fromSide?: "top" | "right" | "bottom" | "left",
    toSide?: "top" | "right" | "bottom" | "left",
  ) => {
    const edge = {
      id: `edge-${crypto.randomUUID()}`,
      fromNode,
      toNode,
      fromSide,
      toSide,
    };
    const currentAdapter = adapterRef.current;
    if (!currentAdapter) {
      fitNextLoadRef.current = false;
      setData((current) => (current ? previewAddEdge(current, edge) : current));
      return;
    }
    void currentAdapter
      .addEdge({
        edgeId: edge.id,
        fromNode: edge.fromNode,
        toNode: edge.toNode,
        fromSide: edge.fromSide,
        toSide: edge.toSide,
        baseRevision: revisionRef.current,
      })
      .then((revision) => {
        commitRevision(revision);
        fitNextLoadRef.current = false;
        setData((current) => (current ? previewAddEdge(current, edge) : current));
      })
      .catch((error: unknown) =>
        reportError(
          error instanceof CanvasStaleRevisionError
            ? `Canvas changed externally: ${error.message}`
            : String(error),
        ),
      );
  };

  const handleSelectNode = (id: string | null) => {
    if (connectModeRef.current) {
      if (!id) {
        setConnectFromId(null);
        setSelectedId(null);
        return;
      }
      const from = connectFromIdRef.current;
      if (!from) {
        setConnectFromId(id);
        setSelectedId(id);
        return;
      }
      if (from === id) return;
      connectFromIdRef.current = null;
      connectModeRef.current = false;
      connectNodes(from, id);
      setConnectFromId(null);
      setConnectMode(false);
      setSelectedId(id);
      return;
    }
    setSelectedId(id);
    setSelectedEdgeId(null);
  };

  const commitTextEdit = (id: string, text: string) => {
    const next = text.trim() || "New note";
    setTextEdit(null);
    const currentAdapter = adapterRef.current;
    if (!currentAdapter) {
      fitNextLoadRef.current = false;
      setData((current) => (current ? previewUpdateTextNode(current, id, next) : current));
      return;
    }
    void currentAdapter
      .updateTextNode(id, next, revisionRef.current)
      .then((revision) => {
        commitRevision(revision);
        fitNextLoadRef.current = false;
        setData((current) => (current ? previewUpdateTextNode(current, id, next) : current));
      })
      .catch((error: unknown) =>
        reportError(
          error instanceof CanvasStaleRevisionError
            ? `Canvas changed externally: ${error.message}`
            : String(error),
        ),
      );
  };

  // Pixi scene is long-lived across local edits; only recreate when the host mounts.
  const hasCanvasHost = data !== null;
  useEffect(() => {
    if (!hasCanvasHost) return;
    const host = hostRef.current;
    if (!host) return;

    const scene = new CanvasScene(host, {
      onOpenFile: (path, subpath) => onOpenFileRef.current(path, subpath),
      onSelectNode: handleSelectNode,
      onSelectEdge: (id) => {
        setSelectedEdgeId(id);
        if (id) setSelectedId(null);
      },
      onConnectNodes: ({ fromNode, toNode, fromSide, toSide }) => {
        connectNodes(fromNode, toNode, fromSide, toSide);
      },
      onMoveNodes: (nodes) => {
        const currentAdapter = adapterRef.current;
        if (!currentAdapter) return;
        void currentAdapter.moveNodes(nodes, revisionRef.current).then((revision) => {
          commitRevision(revision);
          fitNextLoadRef.current = false;
          setData((current) => (current ? previewMoveNodes(current, nodes) : current));
        }).catch((error: unknown) => {
          reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error));
        });
      },
      onResizeNodes: (nodes) => {
        const currentAdapter = adapterRef.current;
        if (!currentAdapter) {
          fitNextLoadRef.current = false;
          setData((current) => (current ? previewResizeNodes(current, nodes) : current));
          return;
        }
        void currentAdapter.resizeNodes(nodes, revisionRef.current).then((revision) => {
          commitRevision(revision);
          fitNextLoadRef.current = false;
          setData((current) => (current ? previewResizeNodes(current, nodes) : current));
        }).catch((error: unknown) => {
          reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error));
        });
      },
      onRemoveNodes: (nodeIds) => {
        const currentAdapter = adapterRef.current;
        if (!currentAdapter) {
          fitNextLoadRef.current = false;
          setData((current) => current ? {
            nodes: current.nodes.filter((node) => !nodeIds.includes(node.id)),
            edges: current.edges.filter((edge) => !nodeIds.includes(edge.fromNode) && !nodeIds.includes(edge.toNode)),
          } : current);
          return;
        }
        void currentAdapter.removeNodes(nodeIds, revisionRef.current).then((revision) => {
          commitRevision(revision);
          fitNextLoadRef.current = false;
          setData((current) => current ? {
            nodes: current.nodes.filter((node) => !nodeIds.includes(node.id)),
            edges: current.edges.filter((edge) => !nodeIds.includes(edge.fromNode) && !nodeIds.includes(edge.toNode)),
          } : current);
        }).catch((error: unknown) => {
          reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error));
        });
      },
      onRemoveEdges: (edgeIds) => {
        const currentAdapter = adapterRef.current;
        if (!currentAdapter) {
          fitNextLoadRef.current = false;
          setSelectedEdgeId(null);
          setData((current) => (current ? previewRemoveEdges(current, edgeIds) : current));
          return;
        }
        void currentAdapter.removeEdges(edgeIds, revisionRef.current).then((revision) => {
          commitRevision(revision);
          fitNextLoadRef.current = false;
          setSelectedEdgeId(null);
          setData((current) => (current ? previewRemoveEdges(current, edgeIds) : current));
        }).catch((error: unknown) => {
          reportError(error instanceof CanvasStaleRevisionError ? `Canvas changed externally: ${error.message}` : String(error));
        });
      },
      onEditText: (nodeId, text) => {
        setTextEdit({ id: nodeId, text });
      },
    });
    sceneRef.current = scene;
    // Scene remounts without a `data` identity change leave Pixi empty unless we
    // re-apply the latest snapshot when the new scene becomes ready.
    void scene.ready.then(() => {
      if (sceneRef.current !== scene) return;
      const snapshot = dataRef.current;
      if (!snapshot) return;
      const fit = fitNextLoadRef.current;
      fitNextLoadRef.current = false;
      scene.setData(snapshot, { fit });
    });
    return () => {
      sceneRef.current = null;
      scene.destroy();
    };
  }, [hasCanvasHost]);

  useEffect(() => {
    if (!data) return;
    const scene = sceneRef.current;
    if (!scene) return;
    const fit = fitNextLoadRef.current;
    fitNextLoadRef.current = false;
    void scene.ready.then(() => {
      if (sceneRef.current === scene) scene.setData(data, { fit });
    });
  }, [data]);

  if (parsed.error) {
    return (
      <div className="placeholder">
        <p className="placeholder-copy">Couldn't parse this canvas.</p>
        <p className="placeholder-sub"><code>{parsed.error}</code></p>
      </div>
    );
  }
  if (!data) return null;

  const previewPath = (resourcePath: string) => {
    try {
      return canvasRelativePath(canvasPath, resourcePath);
    } catch {
      return resourcePath;
    }
  };

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
      commitRevision(revision);
      setData((current) => current ? {
        nodes: current.nodes.filter((node) => node.id !== id),
        edges: current.edges.filter((edge) => edge.fromNode !== id && edge.toNode !== id),
      } : current);
    }).catch((error: unknown) => reportError(String(error)));
  };

  const placeResourceAt = (resourcePath: string, x = 120, y = 120) => {
    const node = {
      id: `resource-${crypto.randomUUID()}`,
      x,
      y,
      width: 320,
      height: 200,
    };
    const file = previewPath(resourcePath);
    const currentAdapter = adapterRef.current;
    if (!currentAdapter) {
      fitNextLoadRef.current = false;
      setData((current) => (current ? previewPlaceResource(current, file, node) : current));
      setPlaceOpen(false);
      return;
    }
    void currentAdapter
      .placeResource({
        resourcePath,
        nodeId: node.id,
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
        baseRevision: revisionRef.current,
      })
      .then((revision) => {
        commitRevision(revision);
        fitNextLoadRef.current = false;
        setData((current) => (current ? previewPlaceResource(current, file, node) : current));
        setPlaceOpen(false);
        setPlaceQuery("");
      })
      .catch((error: unknown) =>
        reportError(
          error instanceof CanvasStaleRevisionError
            ? `Canvas changed externally: ${error.message}`
            : String(error),
        ),
      );
  };

  const addTextNoteAt = (x = 120, y = 120) => {
    const node = {
      id: `text-${crypto.randomUUID()}`,
      text: "New note",
      x,
      y,
      width: DEFAULT_NOTE_WIDTH,
      height: DEFAULT_NOTE_HEIGHT,
    };
    const currentAdapter = adapterRef.current;
    if (!currentAdapter) {
      fitNextLoadRef.current = false;
      setData((current) => (current ? previewAddTextNode(current, node) : current));
      setTextEdit({ id: node.id, text: node.text });
      return;
    }
    void currentAdapter
      .addTextNode({
        nodeId: node.id,
        text: node.text,
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
        baseRevision: revisionRef.current,
      })
      .then((revision) => {
        commitRevision(revision);
        fitNextLoadRef.current = false;
        setData((current) => (current ? previewAddTextNode(current, node) : current));
        setTextEdit({ id: node.id, text: node.text });
      })
      .catch((error: unknown) =>
        reportError(
          error instanceof CanvasStaleRevisionError
            ? `Canvas changed externally: ${error.message}`
            : String(error),
        ),
      );
  };

  return (
    <div
      className={`canvas-surface${outlineOpen ? "" : " is-outline-collapsed"}`}
      tabIndex={0}
      onDragOver={(event) => {
        if (
          event.dataTransfer?.types.includes(LATTICE_RESOURCE_MIME) ||
          (event.dataTransfer?.files?.length ?? 0) > 0
        ) {
          event.preventDefault();
          if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
        }
      }}
      onDrop={(event) => {
        const payload = readResourceDragPayload(event.dataTransfer);
        if (payload) {
          event.preventDefault();
          const point = dropWorldPoint(event.clientX, event.clientY);
          placeResourceAt(payload.path, point.x, point.y);
          return;
        }
        const files = Array.from(event.dataTransfer?.files ?? []);
        if (files.length > 0) {
          event.preventDefault();
          reportError("Import OS files onto the canvas from a page first, then drag the imported resource.");
        }
      }}
      onKeyDown={(event) => {
        if (textEdit) {
          if (event.key === "Escape") {
            setTextEdit(null);
            event.preventDefault();
          }
          return;
        }
        if (event.key === "Escape") {
          if (placeOpen) {
            setPlaceOpen(false);
            event.preventDefault();
            return;
          }
          if (connectMode) {
            setConnectMode(false);
            setConnectFromId(null);
            event.preventDefault();
            return;
          }
        }
        if (connectMode) return;
        const delta = keyboardMoveDelta(event.key, event.shiftKey);
        if (delta && sceneRef.current?.moveSelectedBy(delta.x, delta.y)) event.preventDefault();
        if ((event.key === "Delete" || event.key === "Backspace") && sceneRef.current?.removeSelected()) {
          event.preventDefault();
        }
      }}
    >
      <div className="canvas-main">
        <div className="canvas-toolbar" aria-label="Canvas editing actions">
          <button
            type="button"
            className={placeOpen ? "is-active" : undefined}
            aria-pressed={placeOpen}
            onClick={() => {
              setPlaceOpen((open) => !open);
              setConnectMode(false);
              setConnectFromId(null);
            }}
          >
            Place resource
          </button>
          <button
            type="button"
            onClick={() => {
              const point = dropWorldPoint(
                (hostRef.current?.getBoundingClientRect().left ?? 0) + 160,
                (hostRef.current?.getBoundingClientRect().top ?? 0) + 120,
              );
              addTextNoteAt(point.x, point.y);
              setPlaceOpen(false);
              setConnectMode(false);
            }}
          >
            Add note
          </button>
          <button
            type="button"
            className={connectMode ? "is-active" : undefined}
            aria-pressed={connectMode}
            onClick={() => {
              setConnectMode((open) => !open);
              setConnectFromId(null);
              setPlaceOpen(false);
            }}
          >
            Connect
          </button>
          <button type="button" onClick={() => sceneRef.current?.removeSelected()}>
            {selectedEdgeId ? "Delete edge" : "Remove"}
          </button>
          <button type="button" onClick={() => sceneRef.current?.fitView()}>
            Fit
          </button>
          <button
            type="button"
            className={outlineOpen ? "is-active" : undefined}
            aria-pressed={outlineOpen}
            onClick={() => setOutlineOpenPersisted(!outlineOpen)}
          >
            Outline
          </button>
          <span className="canvas-toolbar-hint">
            {connectMode
              ? connectFromId
                ? "Click a second node to draw an arrow"
                : "Click the first node to connect"
              : selectedEdgeId
                ? "Press Delete to remove the selected edge"
                : "Drag ports to connect · SE corner to resize · drop resources under pan/zoom"}
          </span>
        </div>
        {placeOpen && (
          <div className="canvas-place-panel" role="dialog" aria-label="Place resource on canvas">
            <input
              className="canvas-place-filter"
              type="search"
              value={placeQuery}
              placeholder="Filter workspace resources…"
              autoFocus
              onChange={(event) => setPlaceQuery(event.target.value)}
            />
            <ul className="canvas-place-list">
              {placeableResources.length === 0 ? (
                <li className="canvas-place-empty">No matching resources.</li>
              ) : (
                placeableResources.map((resource) => (
                  <li key={resource.path}>
                    <button type="button" onClick={() => placeResourceAt(resource.path)}>
                      <span>{fileLabel(resource.path)}</span>
                      <span className="canvas-place-path">{resource.path}</span>
                    </button>
                  </li>
                ))
              )}
            </ul>
          </div>
        )}
        {textEdit && (
          <div className="canvas-text-editor" role="dialog" aria-label="Edit sticky note">
            <textarea
              value={textEdit.text}
              autoFocus
              rows={5}
              onChange={(event) => setTextEdit({ id: textEdit.id, text: event.target.value })}
              onKeyDown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                  event.preventDefault();
                  commitTextEdit(textEdit.id, textEdit.text);
                }
              }}
            />
            <div className="canvas-text-editor-actions">
              <button type="button" onClick={() => setTextEdit(null)}>Cancel</button>
              <button type="button" onClick={() => commitTextEdit(textEdit.id, textEdit.text)}>
                Save note
              </button>
            </div>
          </div>
        )}
        {errorMessage && <p className="canvas-conflict" role="alert">{errorMessage}</p>}
        <div ref={hostRef} className="canvas-viewer" />
      </div>
      {outlineOpen && (
        <CanvasOutline
          nodes={data.nodes}
          selectedId={selectedId}
          onSelect={(id) => {
            sceneRef.current?.selectNode(id);
          }}
          onRemove={removeFromOutline}
          onClose={() => setOutlineOpenPersisted(false)}
        />
      )}
    </div>
  );
}
