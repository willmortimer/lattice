import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { demoTextFiles, inBrowser } from "../../demo";
import type { Resource } from "../../types";
import type { ResourceRendererContext } from "../../renderers/RendererContext";
import { formatBytes, MAX_IMAGE_DECODED_PIXELS } from "./mediaLimits";
import { createObjectUrlLease, loadImageAsset, type ImageAsset } from "./imageSource";
import { readImageDimensions } from "./imageMetadata";
import { MediaDegraded } from "./MediaDegraded";
import "./media.css";

const MIN_ZOOM = 0.05;
const MAX_ZOOM = 8;

function clampZoom(value: number): number { return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, value)); }

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Browser demo fixture for SVG seeds without native filesystem reads. */
function loadDemoSvgAsset(path: string): ImageAsset {
  const content = demoTextFiles[path];
  if (!content) {
    throw new Error(`Demo image fixture missing for ${path}`);
  }
  const bytes = new TextEncoder().encode(content);
  const dimensions = readImageDimensions(bytes);
  const blobBytes = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
  const lease = createObjectUrlLease(new Blob([blobBytes], { type: "image/svg+xml" }));
  return {
    lease,
    dimensions,
    encodedBytes: bytes.byteLength,
    mimeType: "image/svg+xml",
  };
}

export function ImageViewer({ context, resource }: { context: ResourceRendererContext; resource: Resource }) {
  const [asset, setAsset] = useState<ImageAsset | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [mode, setMode] = useState<"fit" | "actual">("fit");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [stageSize, setStageSize] = useState({ width: 0, height: 0 });
  const stageRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointerId: number; x: number; y: number; panX: number; panY: number } | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let loaded: ImageAsset | null = null;
    setAsset(null);
    setError(null);
    setNaturalSize(null);
    setPan({ x: 0, y: 0 });
    setMode("fit");
    const load = context.workspaceRoot
      ? loadImageAsset({ root: context.workspaceRoot, path: resource.path }, controller.signal)
      : inBrowser && resource.path.toLowerCase().endsWith(".svg")
        ? Promise.resolve(loadDemoSvgAsset(resource.path))
        : Promise.reject(new Error("Native media reads are unavailable in browser preview."));
    void load
      .then((next) => {
        if (controller.signal.aborted) {
          next.lease.revoke();
          return;
        }
        loaded = next;
        setAsset(next);
        if (next.dimensions?.width && next.dimensions.height) setNaturalSize(next.dimensions);
      })
      .catch((nextError: unknown) => {
        if (!controller.signal.aborted) setError(errorMessage(nextError));
      });
    return () => {
      controller.abort();
      loaded?.lease.revoke();
    };
  }, [context.workspaceRoot, resource.path]);

  const fitZoom = useMemo(() => {
    if (!naturalSize || stageSize.width <= 0 || stageSize.height <= 0) return 1;
    return Math.min(1, stageSize.width / naturalSize.width, stageSize.height / naturalSize.height);
  }, [naturalSize, stageSize]);

  const effectiveZoom = mode === "fit" ? fitZoom : zoom;
  const setZoomMode = useCallback((nextMode: "fit" | "actual") => {
    setMode(nextMode);
    if (nextMode === "actual") setZoom(1);
    setPan({ x: 0, y: 0 });
  }, []);
  const adjustZoom = useCallback((delta: number) => {
    setMode("actual");
    setZoom((value) => clampZoom(value * delta));
  }, []);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const update = () => setStageSize({ width: stage.clientWidth, height: stage.clientHeight });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(stage);
    return () => observer.disconnect();
  }, [asset]);

  // React's onWheel is passive in modern browsers, so preventDefault there
  // cannot stop .main-scroll from also consuming the gesture.
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      adjustZoom(event.deltaY < 0 ? 1.1 : 0.9);
    };
    stage.addEventListener("wheel", onWheel, { passive: false });
    return () => stage.removeEventListener("wheel", onWheel);
  }, [adjustZoom, asset]);

  const onImageLoad = (event: React.SyntheticEvent<HTMLImageElement>) => {
    const image = event.currentTarget;
    const width = image.naturalWidth;
    const height = image.naturalHeight;
    if (width * height > MAX_IMAGE_DECODED_PIXELS) {
      asset?.lease.revoke();
      setAsset(null);
      setError(`This image is too large to decode safely (${(width * height).toLocaleString()} pixels; limit ${MAX_IMAGE_DECODED_PIXELS.toLocaleString()}).`);
      return;
    }
    setNaturalSize({ width, height });
  };

  if (error) return <MediaDegraded context={context} resource={resource} title="Image preview unavailable" message={error} />;
  if (!asset) return <div className="media-viewer media-degraded" aria-live="polite">Loading image…</div>;

  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y };
  };
  const onPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setPan({ x: drag.panX + event.clientX - drag.x, y: drag.panY + event.clientY - drag.y });
  };
  const stopPointer = (event: React.PointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null;
  };

  return (
    <div className="media-viewer image-viewer">
      <div className="media-toolbar" role="toolbar" aria-label="Image viewer controls">
        <div className="media-toolbar-group">
          <button className={`media-button${mode === "fit" ? " media-button-active" : ""}`} type="button" onClick={() => setZoomMode("fit")}>Fit</button>
          <button className={`media-button${mode === "actual" && zoom === 1 ? " media-button-active" : ""}`} type="button" onClick={() => setZoomMode("actual")}>Actual</button>
          <button className="media-button" type="button" onClick={() => adjustZoom(0.8)} aria-label="Zoom out">−</button>
          <span className="media-toolbar-status" aria-live="polite">{Math.round(effectiveZoom * 100)}%</span>
          <button className="media-button" type="button" onClick={() => adjustZoom(1.25)} aria-label="Zoom in">+</button>
        </div>
        <span className="media-toolbar-spacer" />
        <span className="media-toolbar-status">
          {naturalSize ? `${naturalSize.width} × ${naturalSize.height}` : "Dimensions unavailable"} · {formatBytes(asset.encodedBytes)}
        </span>
      </div>
      <div
        ref={stageRef}
        className="image-stage"
        role="application"
        aria-label={`Image viewer for ${resource.path}. Drag to pan; use the mouse wheel to zoom.`}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={stopPointer}
        onPointerCancel={stopPointer}
      >
        <div className="image-canvas">
          <img
            className="image-content"
            src={asset.lease.url}
            alt={resource.path}
            onLoad={onImageLoad}
            style={{
              width: naturalSize ? naturalSize.width : undefined,
              height: naturalSize ? naturalSize.height : undefined,
              transform: `translate3d(${pan.x}px, ${pan.y}px, 0) scale(${effectiveZoom})`,
            }}
          />
        </div>
        <span className="image-hint">Drag to pan · wheel to zoom</span>
      </div>
    </div>
  );
}
