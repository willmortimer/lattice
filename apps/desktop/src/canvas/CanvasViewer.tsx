import { useEffect, useMemo, useRef, useState } from "react";
import type { Doc } from "yjs";

import type { PagePersistMode } from "../editor/collab/collabSession";
import { LATTICE_RESOURCE_MIME, readResourceDragPayload } from "../lib/resourceDrag";
import { readTextWindow } from "../lib/resourceRuntime";
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
import { canvasDataFromYDoc, observeCanvasYDoc } from "./collab/canvasYDoc";
import { CanvasScene } from "./scene";
import { CanvasParseError, parseCanvas, type CanvasData } from "./types";
import {
  canvasPresentationSidecarPath,
  createCanvasPresentationSession,
  extractEmbeddedCanvasPresentation,
  parseCanvasPresentationManifest,
  resolveCanvasSceneIndex,
  resolveCanvasScenes,
  type CanvasPresentationManifest,
  type CanvasSceneSpec,
} from "../presentation/presentationSession";
import type { Resource } from "../types";

const OUTLINE_OPEN_KEY = "lattice.canvas.outlineOpen";
const DEFAULT_NOTE_WIDTH = 200;
const DEFAULT_NOTE_HEIGHT = 140;
const PRESENT_CAMERA_MS = 480;
const SIDECAR_READ_BYTES = 256_000;

const PERSIST_LABELS: Record<PagePersistMode, string> = {
  plain: "Plain file",
  collaborative: "Collaborative",
};

interface CanvasViewerProps {
  json: unknown;
  canvasPath: string;
  workspaceRoot?: string;
  resources?: readonly Resource[];
  onOpenFile: (path: string, subpath?: string) => void;
  adapter?: CanvasAdapter;
  baseRevision: string;
  onRevisionChange?: (revision: string) => void;
  onError?: (message: string) => void;
  persistMode?: PagePersistMode;
  collaborativeAvailable?: boolean;
  onPersistModeChange?: (mode: PagePersistMode) => void;
  collabYdoc?: Doc | null;
  collabLoading?: boolean;
  collabError?: string | null;
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

function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false,
  );
  useEffect(() => {
    const query = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!query) return;
    const change = () => setReduced(query.matches);
    query.addEventListener("change", change);
    return () => query.removeEventListener("change", change);
  }, []);
  return reduced;
}

/** Pixi owns the scene hot loop; the DOM outline remains the accessible action surface. */
export function CanvasViewer({
  json,
  canvasPath,
  workspaceRoot,
  resources = [],
  onOpenFile,
  adapter,
  baseRevision,
  onRevisionChange,
  onError,
  persistMode = "plain",
  collaborativeAvailable = false,
  onPersistModeChange,
  collabYdoc = null,
  collabLoading = false,
  collabError = null,
}: CanvasViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const onOpenFileRef = useRef(onOpenFile);
  const adapterRef = useRef(adapter);
  const revisionRef = useRef(baseRevision);
  const onRevisionChangeRef = useRef(onRevisionChange);
  const onErrorRef = useRef(onError);
  const sceneRef = useRef<CanvasScene | null>(null);
  const fitNextLoadRef = useRef(true);
  const connectModeRef = useRef(false);
  const connectFromIdRef = useRef<string | null>(null);
  const presentingRef = useRef(false);
  const sceneIndexRef = useRef(0);
  const scenesRef = useRef<CanvasSceneSpec[]>([]);
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
  const [zoomPct, setZoomPct] = useState(100);
  const [manifest, setManifest] = useState<CanvasPresentationManifest | null>(() =>
    extractEmbeddedCanvasPresentation(json),
  );
  const [presenting, setPresenting] = useState(false);
  const [sceneIndex, setSceneIndex] = useState(0);
  const reducedMotion = useReducedMotion();

  connectModeRef.current = connectMode;
  connectFromIdRef.current = connectFromId;
  dataRef.current = data;
  presentingRef.current = presenting;
  sceneIndexRef.current = sceneIndex;

  useEffect(() => {
    revisionRef.current = baseRevision;
  }, [baseRevision]);

  useEffect(() => {
    let cancelled = false;
    const embedded = extractEmbeddedCanvasPresentation(json);
    if (!workspaceRoot) {
      setManifest(embedded);
      return;
    }
    const sidecarPath = canvasPresentationSidecarPath(canvasPath);
    void readTextWindow({
      root: workspaceRoot,
      path: sidecarPath,
      offset: 0,
      length: SIDECAR_READ_BYTES,
    })
      .then((window) => {
        if (cancelled) return;
        setManifest(parseCanvasPresentationManifest(JSON.parse(window.content)));
      })
      .catch(() => {
        if (!cancelled) setManifest(embedded);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot, canvasPath, json]);

  const scenes = useMemo(
    () => resolveCanvasScenes(manifest, data?.nodes ?? []),
    [manifest, data],
  );
  scenesRef.current = scenes;

  const presentation = useMemo(
    () =>
      createCanvasPresentationSession(
        canvasPath,
        manifest?.title ?? fileLabel(canvasPath),
        scenes,
        { start: manifest?.start },
      ),
    [canvasPath, manifest, scenes],
  );

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
    if (collabYdoc) return;
    fitNextLoadRef.current = true;
    setData(parsed.data);
    setSelectedId(null);
    setSelectedEdgeId(null);
    setConnectFromId(null);
    setTextEdit(null);
    setPresenting(false);
    setSceneIndex(0);
  }, [collabYdoc, parsed.data]);

  useEffect(() => {
    if (!collabYdoc) return;
    const applyLive = () => {
      fitNextLoadRef.current = false;
      setData(canvasDataFromYDoc(collabYdoc));
    };
    applyLive();
    return observeCanvasYDoc(collabYdoc, applyLive);
  }, [collabYdoc]);

  const frameScene = (index: number, animate: boolean) => {
    const scene = scenesRef.current[index];
    const pixi = sceneRef.current;
    if (!scene || !pixi) return;
    const durationMs = animate && !reducedMotion ? PRESENT_CAMERA_MS : 0;
    if (scene.viewport) {
      void pixi.frameBounds(scene.viewport, {
        padding: scene.viewport.padding ?? 48,
        durationMs,
      });
    } else if (scene.nodeIds?.length) {
      void pixi.frameNodes(scene.nodeIds, { durationMs });
      const focus = scene.nodeIds[0];
      if (focus) pixi.selectNode(focus);
    }
  };

  const goToScene = (next: number, animate: boolean) => {
    const count = scenesRef.current.length;
    if (count === 0) return;
    const index = Math.max(0, Math.min(count - 1, next));
    setSceneIndex(index);
    sceneIndexRef.current = index;
    frameScene(index, animate);
  };

  const exitPresent = () => {
    setPresenting(false);
    presentingRef.current = false;
    if (document.fullscreenElement && document.fullscreenElement === surfaceRef.current) {
      void document.exitFullscreen();
    }
  };

  const enterPresent = () => {
    setPlaceOpen(false);
    setConnectMode(false);
    setConnectFromId(null);
    setTextEdit(null);
    setPresenting(true);
    presentingRef.current = true;
    const initial = resolveCanvasSceneIndex(presentation.orderedIds, presentation.initialId);
    setSceneIndex(initial);
    sceneIndexRef.current = initial;
    // Wait a frame so fullscreen layout can resize the Pixi host first.
    requestAnimationFrame(() => {
      frameScene(initial, false);
      void surfaceRef.current?.requestFullscreen?.().catch(() => {
        // Fullscreen may be blocked; present mode still works in-pane.
      });
      requestAnimationFrame(() => frameScene(initial, false));
    });
  };

  useEffect(() => {
    const onFullscreen = () => {
      if (!presentingRef.current) return;
      if (document.fullscreenElement !== surfaceRef.current) {
        setPresenting(false);
        presentingRef.current = false;
      }
    };
    document.addEventListener("fullscreenchange", onFullscreen);
    return () => document.removeEventListener("fullscreenchange", onFullscreen);
  }, []);

  useEffect(() => {
    if (!presenting) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (["ArrowRight", "ArrowDown", "PageDown", " ", "Enter"].includes(event.key)) {
        event.preventDefault();
        goToScene(sceneIndexRef.current + 1, true);
      } else if (["ArrowLeft", "ArrowUp", "PageUp", "Backspace"].includes(event.key)) {
        event.preventDefault();
        goToScene(sceneIndexRef.current - 1, true);
      } else if (event.key === "Home") {
        event.preventDefault();
        goToScene(0, true);
      } else if (event.key === "End") {
        event.preventDefault();
        goToScene(scenesRef.current.length - 1, true);
      } else if (event.key === "Escape") {
        event.preventDefault();
        exitPresent();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [presenting, reducedMotion, presentation.orderedIds]);

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
    const unsubscribeZoom = scene.onZoomChange((zoom) => {
      setZoomPct(Math.round(zoom * 100));
    });
    // Scene remounts without a `data` identity change leave Pixi empty unless we
    // re-apply the latest snapshot when the new scene becomes ready.
    void scene.ready
      .then(() => {
        if (sceneRef.current !== scene) return;
        const snapshot = dataRef.current;
        if (!snapshot) return;
        const fit = fitNextLoadRef.current;
        fitNextLoadRef.current = false;
        scene.setData(snapshot, { fit });
      })
      .catch((error: unknown) => {
        reportError(
          error instanceof Error
            ? `Canvas renderer failed: ${error.message}`
            : `Canvas renderer failed: ${String(error)}`,
        );
      });
    return () => {
      unsubscribeZoom();
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
    void scene.ready
      .then(() => {
        if (sceneRef.current === scene) scene.setData(data, { fit });
      })
      .catch((error: unknown) => {
        reportError(
          error instanceof Error
            ? `Canvas renderer failed: ${error.message}`
            : `Canvas renderer failed: ${String(error)}`,
        );
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
      ref={surfaceRef}
      className={`canvas-surface${outlineOpen && !presenting ? "" : " is-outline-collapsed"}${presenting ? " is-presenting" : ""}`}
      tabIndex={0}
      onDragOver={(event) => {
        if (presenting) return;
        if (
          event.dataTransfer?.types.includes(LATTICE_RESOURCE_MIME) ||
          (event.dataTransfer?.files?.length ?? 0) > 0
        ) {
          event.preventDefault();
          if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
        }
      }}
      onDrop={(event) => {
        if (presenting) return;
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
        if (presenting) return;
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
        if (
          !event.metaKey &&
          !event.ctrlKey &&
          !event.altKey &&
          (event.key === "p" || event.key === "P") &&
          scenes.length > 0
        ) {
          event.preventDefault();
          enterPresent();
          return;
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
        {!presenting && (
        <div className="canvas-toolbar" aria-label="Canvas editing actions">
          {collaborativeAvailable && onPersistModeChange ? (
            <span className="canvas-persist-tabs" role="radiogroup" aria-label="Canvas persistence mode">
              {(Object.keys(PERSIST_LABELS) as PagePersistMode[]).map((candidate) => (
                <button
                  key={candidate}
                  type="button"
                  role="radio"
                  aria-checked={persistMode === candidate}
                  className={persistMode === candidate ? "is-active" : undefined}
                  onClick={() => onPersistModeChange(candidate)}
                >
                  {PERSIST_LABELS[candidate]}
                </button>
              ))}
            </span>
          ) : null}
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
          <button
            type="button"
            className={outlineOpen ? "is-active" : undefined}
            aria-pressed={outlineOpen}
            onClick={() => setOutlineOpenPersisted(!outlineOpen)}
          >
            Outline
          </button>
          <button
            type="button"
            disabled={scenes.length === 0}
            title="Present (P)"
            onClick={() => enterPresent()}
          >
            Present
          </button>
          <span className="canvas-toolbar-hint">
            {connectMode
              ? connectFromId
                ? "Click a second node to draw an arrow"
                : "Click the first node to connect"
              : selectedEdgeId
                ? "Press Delete to remove the selected edge"
                : "Drag ports to connect · SE corner to resize · drop resources under pan/zoom · P to present"}
          </span>
          <button
            type="button"
            aria-label="Zoom out"
            onClick={() => {
              const scene = sceneRef.current;
              if (scene) scene.setZoom(scene.getZoom() / 1.25);
            }}
          >
            −
          </button>
          <button
            type="button"
            aria-label="Reset zoom to 100%"
            title="Reset zoom"
            style={{ minWidth: 52, fontVariantNumeric: "tabular-nums" }}
            onClick={() => sceneRef.current?.setZoom(1)}
          >
            {zoomPct}%
          </button>
          <button
            type="button"
            aria-label="Zoom in"
            onClick={() => {
              const scene = sceneRef.current;
              if (scene) scene.setZoom(scene.getZoom() * 1.25);
            }}
          >
            +
          </button>
          <button type="button" onClick={() => sceneRef.current?.fitView()}>
            Fit
          </button>
        </div>
        )}
        {presenting && (
          <div className="canvas-present-chrome" role="toolbar" aria-label="Presentation controls">
            <button type="button" onClick={() => goToScene(sceneIndex - 1, true)} disabled={sceneIndex <= 0}>
              Previous
            </button>
            <output aria-live="polite">
              {sceneIndex + 1} / {scenes.length}
              {scenes[sceneIndex]?.title ? ` · ${scenes[sceneIndex]?.title}` : scenes[sceneIndex]?.id ? ` · ${scenes[sceneIndex]?.id}` : ""}
            </output>
            <button
              type="button"
              onClick={() => goToScene(sceneIndex + 1, true)}
              disabled={sceneIndex >= scenes.length - 1}
            >
              Next
            </button>
            <button type="button" onClick={() => exitPresent()}>
              Exit
            </button>
            <span className="canvas-toolbar-hint">← → to advance · Esc to exit</span>
          </div>
        )}
        {placeOpen && !presenting && (
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
        {textEdit && !presenting && (
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
        {collabLoading && persistMode === "collaborative" && (
          <p className="canvas-toolbar-hint" role="status">Opening collaborative canvas…</p>
        )}
        {collabError && persistMode === "collaborative" && (
          <p className="canvas-conflict" role="alert">{collabError}</p>
        )}
        {errorMessage && <p className="canvas-conflict" role="alert">{errorMessage}</p>}
        <div ref={hostRef} className="canvas-viewer" />
      </div>
      {outlineOpen && !presenting && (
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
