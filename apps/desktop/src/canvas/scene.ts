// Tauri CSP omits script unsafe-eval (Vega uses a CSP-safe interpreter). Pixi's
// default shader path needs this polyfill before any other pixi.js import.
import "pixi.js/unsafe-eval";
import { Application, Container, FederatedPointerEvent, Graphics, Rectangle, Text } from "pixi.js";
import type { CanvasNodeMove, CanvasNodeSize } from "./adapter";
import type { CanvasData, CanvasEdge, CanvasNode } from "./types";
import type { ResourceKind } from "../types";
import { classifyPath } from "./classify";
import { KIND_LABELS } from "../KindMark";
import { hexToRgba, observeThemeChange, readCanvasPalette, type CanvasPalette } from "./colors";

const MIN_SCALE = 0.1;
const MAX_SCALE = 3;
const DOUBLE_CLICK_MS = 400;
const CARD_RADIUS = 8;
const CARD_PADDING = 12;
const PORT_RADIUS = 5;
const PORT_HIT = 12;
const RESIZE_HANDLE = 10;
const MIN_NODE_SIZE = 80;
const CHIP_SIZE = 22;
const CHIP_RADIUS = 6;
/** Grid stays legible: fade out below this zoom instead of drawing dust. */
const GRID_FADE_LO = 0.32;
const GRID_FADE_HI = 0.55;
/** Hard cap on dots per redraw; spacing doubles until under this. */
const GRID_MAX_DOTS = 6000;
const SIDES = ["top", "right", "bottom", "left"] as const;

interface CanvasConnectRequest {
  fromNode: string;
  toNode: string;
  fromSide: Side;
  toSide: Side;
}

interface CanvasSceneOptions {
  onOpenFile: (path: string, subpath?: string) => void;
  onSelectNode?: (id: string | null) => void;
  onSelectEdge?: (id: string | null) => void;
  onMoveNodes?: (nodes: CanvasNodeMove[]) => void;
  onResizeNodes?: (nodes: CanvasNodeSize[]) => void;
  onRemoveNodes?: (nodeIds: string[]) => void;
  onRemoveEdges?: (edgeIds: string[]) => void;
  onConnectNodes?: (edge: CanvasConnectRequest) => void;
  onEditText?: (nodeId: string, text: string) => void;
}

type Side = "top" | "right" | "bottom" | "left";

interface NodeCard {
  container: Container;
  bg: Graphics;
  node: CanvasNode;
  ports: Map<Side, Graphics>;
  resizeHandle: Graphics;
}

interface GroupCard {
  container: Container;
  bg: Graphics;
  node: CanvasNode & { type: "group" };
}

const SIDE_NORMAL: Record<Side, { x: number; y: number }> = {
  top: { x: 0, y: -1 },
  bottom: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
  right: { x: 1, y: 0 },
};

function basename(path: string): string {
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  return trimmed.split("/").pop() ?? trimmed;
}

function sidePoint(node: CanvasNode, side: Side): { x: number; y: number } {
  switch (side) {
    case "top":
      return { x: node.x + node.width / 2, y: node.y };
    case "bottom":
      return { x: node.x + node.width / 2, y: node.y + node.height };
    case "left":
      return { x: node.x, y: node.y + node.height / 2 };
    case "right":
      return { x: node.x + node.width, y: node.y + node.height / 2 };
  }
}

/** Pick the side of `from` that most directly faces `to`, Obsidian-style. */
function autoSide(from: CanvasNode, to: CanvasNode): Side {
  const fromCx = from.x + from.width / 2;
  const fromCy = from.y + from.height / 2;
  const toCx = to.x + to.width / 2;
  const toCy = to.y + to.height / 2;
  const dx = toCx - fromCx;
  const dy = toCy - fromCy;
  if (Math.abs(dx) > Math.abs(dy)) {
    return dx >= 0 ? "right" : "left";
  }
  return dy >= 0 ? "bottom" : "top";
}

/**
 * Imperative PixiJS v8 scene for a read-only JSON Canvas view: pan, zoom,
 * node selection, and file-node double-click. No React, no editing.
 */
export class CanvasScene {
  private app = new Application();
  private world = new Container();
  /** Camera-tracked dot grid; redrawn (one Graphics) on every camera change. */
  private gridLayer = new Graphics();
  private groupsLayer = new Container();
  private edgesLayer = new Container();
  private nodesLayer = new Container();

  private nodeCards = new Map<string, NodeCard>();
  private groupCards = new Map<string, GroupCard>();
  private edgeGraphics = new Map<string, Graphics>();
  private selectedId: string | null = null;
  private selectedEdgeId: string | null = null;
  private hoveredId: string | null = null;
  private hoveredEdgeId: string | null = null;
  private lastTapAt = new Map<string, number>();
  private suppressTapFor: string | null = null;
  private zoomListeners = new Set<(scale: number) => void>();
  private lastZoom = 1;
  private dragging: {
    id: string;
    container: Container;
    startX: number;
    startY: number;
    nodeX: number;
    nodeY: number;
    /** Group drags carry member cards along by the same delta. */
    members: Array<{ id: string; container: Container; originX: number; originY: number }>;
    moved: boolean;
  } | null = null;
  private resizing: {
    id: string;
    startX: number;
    startY: number;
    width: number;
    height: number;
    moved: boolean;
  } | null = null;
  private linking: {
    fromId: string;
    fromSide: Side;
    preview: Graphics;
  } | null = null;

  private resizeObserver: ResizeObserver | null = null;
  private disconnectThemeObserver: (() => void) | null = null;
  private host: HTMLElement;
  private options: CanvasSceneOptions;
  private palette: CanvasPalette = readCanvasPalette();
  private data: CanvasData | null = null;
  /** Fit deferred until the host has a real (non-1×1) size after layout. */
  private pendingFit = false;
  /** setData before init finished — apply once the renderer exists. */
  private queuedData: { data: CanvasData; fit: boolean } | null = null;
  /** First successful paint should frame content even if a race cleared fit flags. */
  private needsInitialFit = true;

  private panning = false;
  private panStart = { x: 0, y: 0 };
  private panOrigin = { x: 0, y: 0 };

  /** Lifecycle guards: React StrictMode can destroy() before init resolves. */
  private initialized = false;
  private destroyed = false;
  /** Active camera tween; cancelled by a newer tween, user pan/zoom, or destroy. */
  private cameraTween: { raf: number; token: number } | null = null;
  private cameraTweenToken = 0;

  readonly ready: Promise<void>;

  constructor(host: HTMLElement, options: CanvasSceneOptions) {
    this.host = host;
    this.options = options;

    const rect = host.getBoundingClientRect();
    this.ready = this.app
      .init({
        width: Math.max(1, Math.round(rect.width)),
        height: Math.max(1, Math.round(rect.height)),
        backgroundAlpha: 0,
        antialias: true,
        autoDensity: true,
        resolution: window.devicePixelRatio || 1,
        // Prefer WebGL — WKWebView can exhaust contexts after MapLibre; WebGPU
        // fallbacks have been flaky for this shell.
        preference: "webgl",
      })
      .then(async () => {
        // Packaged WKWebView can leave `document.fonts.ready` pending forever
        // (variable fonts / missing faces). Cap the wait so the scene still boots.
        await Promise.race([
          document.fonts.ready.catch(() => undefined),
          new Promise<void>((resolve) => {
            window.setTimeout(resolve, 400);
          }),
        ]);
        if (this.destroyed) {
          // destroy() ran while init was in flight; finish the teardown here,
          // now that the renderer actually exists.
          this.app.destroy(true, { children: true, texture: true });
          return;
        }
        this.initialized = true;
        this.setup();
        if (this.queuedData) {
          const queued = this.queuedData;
          this.queuedData = null;
          this.rebuild(queued.data, { fit: queued.fit });
        }
      });
  }

  private setup() {
    this.host.appendChild(this.app.canvas);
    this.app.canvas.style.display = "block";
    this.app.canvas.style.touchAction = "none";
    this.app.canvas.tabIndex = 0;
    this.app.canvas.setAttribute("aria-label", "Canvas scene");

    this.gridLayer.eventMode = "none";
    this.world.addChild(this.gridLayer, this.groupsLayer, this.edgesLayer, this.nodesLayer);
    this.app.stage.addChild(this.world);

    this.app.stage.eventMode = "static";
    this.app.stage.hitArea = this.app.screen;

    this.app.stage.on("pointerdown", this.onStagePointerDown);
    this.app.stage.on("globalpointermove", this.onStagePointerMove);
    this.app.stage.on("pointerup", this.onStagePointerUp);
    this.app.stage.on("pointerupoutside", this.onStagePointerUp);

    this.app.canvas.addEventListener("wheel", this.onWheel, { passive: false });

    // app.screen is the same Rectangle instance across resizes (mutated in
    // place by renderer.resize), so the stage.hitArea assignment above keeps
    // tracking it without reassignment.
    this.resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      if (width > 0 && height > 0) {
        this.app.renderer.resize(width, height);
        // First layout after open often starts at ~1×1; re-fit once we have space.
        if (this.pendingFit && this.data && width > 8 && height > 8) {
          this.pendingFit = false;
          this.needsInitialFit = false;
          this.fitToContent(this.data.nodes);
        }
        this.syncCamera();
      }
    });
    this.resizeObserver.observe(this.host);
    // RO can miss the first layout in WKWebView; retry fit after paint.
    this.scheduleFitRetry();

    this.disconnectThemeObserver = observeThemeChange(() => {
      this.palette = readCanvasPalette();
      if (this.data) this.rebuild(this.data, { fit: false });
    });
  }

  /** If pendingFit is stuck, re-measure the host after layout settles. */
  private scheduleFitRetry() {
    const attempt = () => {
      if (this.destroyed || !this.pendingFit || !this.data) return;
      const { width, height } = this.host.getBoundingClientRect();
      if (width > 8 && height > 8) {
        this.app.renderer.resize(width, height);
        this.pendingFit = false;
        this.needsInitialFit = false;
        this.fitToContent(this.data.nodes);
      }
    };
    requestAnimationFrame(() => {
      requestAnimationFrame(attempt);
    });
    window.setTimeout(attempt, 50);
    window.setTimeout(attempt, 250);
  }

  setData(data: CanvasData, options: { fit?: boolean } = {}) {
    const fit = options.fit !== false || this.needsInitialFit;
    if (!this.initialized || this.destroyed) {
      this.queuedData = { data, fit };
      return;
    }
    this.rebuild(data, { fit });
  }

  /** Frame all nodes in the viewport (toolbar Fit / recovery after zero-size layout). */
  fitView() {
    if (this.data?.nodes.length) {
      this.pendingFit = false;
      this.needsInitialFit = false;
      this.cancelCameraTween();
      this.fitToContent(this.data.nodes);
    }
  }

  /**
   * Frame a world-space bounds (presentation scenes). When `durationMs` is 0
   * (or reduced motion), the camera jumps; otherwise it eases pan+zoom.
   */
  frameBounds(
    bounds: { x: number; y: number; width: number; height: number },
    options: { padding?: number; durationMs?: number } = {},
  ): Promise<void> {
    if (!this.initialized || this.destroyed) return Promise.resolve();
    const padding = options.padding ?? 48;
    const target = this.cameraForBounds({
      x: bounds.x - padding,
      y: bounds.y - padding,
      width: Math.max(1, bounds.width + padding * 2),
      height: Math.max(1, bounds.height + padding * 2),
    });
    return this.animateCameraTo(target, options.durationMs ?? 0);
  }

  /** Frame the union of the given node ids (missing ids are skipped). */
  frameNodes(
    nodeIds: readonly string[],
    options: { padding?: number; durationMs?: number } = {},
  ): Promise<void> {
    if (!this.data) return Promise.resolve();
    const byId = new Map(this.data.nodes.map((node) => [node.id, node]));
    const nodes = nodeIds.map((id) => byId.get(id)).filter((node): node is CanvasNode => node != null);
    if (nodes.length === 0) return Promise.resolve();
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of nodes) {
      minX = Math.min(minX, node.x);
      minY = Math.min(minY, node.y);
      maxX = Math.max(maxX, node.x + node.width);
      maxY = Math.max(maxY, node.y + node.height);
    }
    return this.frameBounds(
      { x: minX, y: minY, width: maxX - minX, height: maxY - minY },
      options,
    );
  }

  cancelCameraTween() {
    if (!this.cameraTween) return;
    cancelAnimationFrame(this.cameraTween.raf);
    this.cameraTweenToken += 1;
    this.cameraTween = null;
  }

  /** Convert a browser client point into canvas world coordinates (pan/zoom aware). */
  clientToWorld(clientX: number, clientY: number): { x: number; y: number } {
    const rect = this.app.canvas.getBoundingClientRect();
    return this.toWorld(clientX - rect.left, clientY - rect.top);
  }

  /** Current camera zoom (1 = 100%). */
  getZoom(): number {
    return this.world.scale.x;
  }

  /** Zoom about the viewport center (toolbar −/+/reset). */
  setZoom(next: number) {
    if (!this.initialized || this.destroyed) return;
    this.cancelCameraTween();
    const scale = clamp(next, MIN_SCALE, MAX_SCALE);
    const old = this.world.scale.x;
    if (scale === old) return;
    const cx = (this.app.screen.width || this.host.clientWidth) / 2;
    const cy = (this.app.screen.height || this.host.clientHeight) / 2;
    const worldX = (cx - this.world.position.x) / old;
    const worldY = (cy - this.world.position.y) / old;
    this.world.scale.set(scale);
    this.world.position.set(cx - worldX * scale, cy - worldY * scale);
    this.syncCamera();
  }

  /** Subscribe to zoom changes; fires immediately with the current value. */
  onZoomChange(listener: (scale: number) => void): () => void {
    this.zoomListeners.add(listener);
    listener(this.world.scale.x);
    return () => {
      this.zoomListeners.delete(listener);
    };
  }

  /** Keep camera-dependent chrome (grid, zoom readout) in step after any pan/zoom. */
  private syncCamera() {
    this.updateGrid();
    const zoom = this.world.scale.x;
    if (zoom !== this.lastZoom) {
      this.lastZoom = zoom;
      for (const listener of this.zoomListeners) listener(zoom);
    }
  }

  /**
   * Redraw the dot grid for the visible world range only. One Graphics, dot
   * count capped by doubling the spacing, faded out at low zoom so distant
   * views stay calm.
   */
  private updateGrid() {
    const g = this.gridLayer;
    g.clear();
    const scale = this.world.scale.x;
    const fade = clamp((scale - GRID_FADE_LO) / (GRID_FADE_HI - GRID_FADE_LO), 0, 1);
    g.alpha = fade;
    if (fade <= 0) return;
    const screenW = this.app.screen.width || this.host.clientWidth;
    const screenH = this.app.screen.height || this.host.clientHeight;
    if (screenW <= 8 || screenH <= 8) return;

    let spacing = this.palette.GRID_SIZE;
    const worldW = screenW / scale;
    const worldH = screenH / scale;
    let cols = Math.ceil(worldW / spacing) + 2;
    let rows = Math.ceil(worldH / spacing) + 2;
    while (cols * rows > GRID_MAX_DOTS) {
      spacing *= 2;
      cols = Math.ceil(worldW / spacing) + 2;
      rows = Math.ceil(worldH / spacing) + 2;
    }

    const left = -this.world.position.x / scale;
    const top = -this.world.position.y / scale;
    const startX = Math.floor(left / spacing) * spacing;
    const startY = Math.floor(top / spacing) * spacing;
    // Constant on-screen dot size regardless of zoom (the layer is scaled).
    const radius = 1.1 / scale;
    for (let i = 0; i < cols; i += 1) {
      for (let j = 0; j < rows; j += 1) {
        g.circle(startX + i * spacing, startY + j * spacing, radius);
      }
    }
    g.fill(this.palette.GRID_DOT);
  }

  /** JSON Canvas node/edge color: hex passes through, presets "1".."6" map to theme hues. */
  private resolveNodeColor(color?: string): string | null {
    if (!color) return null;
    if (/^#[0-9a-fA-F]{3,8}$/.test(color)) return color;
    return this.palette.PRESETS[color] ?? null;
  }

  private rebuild(data: CanvasData, options: { fit: boolean }) {
    if (!this.initialized || this.destroyed) return;
    this.cancelLink();
    const selectedId = this.selectedId;
    const selectedEdgeId = this.selectedEdgeId;
    const preserveCamera = !options.fit;
    const camera = preserveCamera
      ? { x: this.world.position.x, y: this.world.position.y, scale: this.world.scale.x }
      : null;

    this.data = data;
    this.groupsLayer.removeChildren();
    this.edgesLayer.removeChildren();
    this.nodesLayer.removeChildren();
    this.nodeCards.clear();
    this.groupCards.clear();
    this.edgeGraphics.clear();
    this.selectedId = null;
    this.selectedEdgeId = null;
    this.hoveredId = null;
    this.hoveredEdgeId = null;

    const byId = new Map(data.nodes.map((n) => [n.id, n]));

    for (const node of data.nodes) {
      if (node.type === "group") {
        const group = this.buildGroup(node);
        this.groupsLayer.addChild(group.container);
        this.groupCards.set(node.id, group);
      } else {
        const card = this.buildCard(node);
        this.nodesLayer.addChild(card.container);
        this.nodeCards.set(node.id, card);
      }
    }

    for (const edge of data.edges) {
      const from = byId.get(edge.fromNode);
      const to = byId.get(edge.toNode);
      if (!from || !to) continue;
      const shell = new Container();
      const g = new Graphics();
      g.eventMode = "static";
      g.cursor = "pointer";
      g.on("pointertap", (e: FederatedPointerEvent) => {
        e.stopPropagation();
        this.selectEdge(edge.id);
      });
      g.on("pointerover", () => {
        this.hoveredEdgeId = edge.id;
        this.drawEdge(g, shell, edge, from, to, this.selectedEdgeId === edge.id, true);
      });
      g.on("pointerout", () => {
        if (this.hoveredEdgeId === edge.id) this.hoveredEdgeId = null;
        this.drawEdge(g, shell, edge, from, to, this.selectedEdgeId === edge.id, false);
      });
      shell.addChild(g);
      this.drawEdge(g, shell, edge, from, to, false);
      this.edgesLayer.addChild(shell);
      this.edgeGraphics.set(edge.id, g);
    }

    if (options.fit) {
      const screenW = this.app.screen.width || this.host.clientWidth;
      const screenH = this.app.screen.height || this.host.clientHeight;
      if (screenW <= 8 || screenH <= 8) {
        this.pendingFit = true;
        this.scheduleFitRetry();
      } else {
        this.pendingFit = false;
        this.needsInitialFit = false;
        this.fitToContent(data.nodes);
      }
    } else if (camera) {
      this.world.scale.set(camera.scale);
      this.world.position.set(camera.x, camera.y);
      this.syncCamera();
    }

    if (selectedId && (this.nodeCards.has(selectedId) || this.groupCards.has(selectedId))) {
      this.selectNode(selectedId);
    } else if (selectedEdgeId && this.edgeGraphics.has(selectedEdgeId)) {
      this.selectEdge(selectedEdgeId);
    }
  }

  private buildGroup(node: CanvasNode & { type: "group" }): GroupCard {
    const container = new Container();
    container.position.set(node.x, node.y);

    const bg = new Graphics();
    container.addChild(bg);
    this.paintGroup(bg, node, false, false);

    if (node.label) {
      const label = new Text({
        text: node.label,
        style: {
          fontFamily: this.palette.FONT_UI,
          fontSize: 12,
          fontWeight: "600",
          letterSpacing: 0.2,
          fill: this.palette.TEXT_SOFT,
        },
      });
      const padX = 8;
      const padY = 4;
      const backdrop = new Graphics()
        .roundRect(10, 10, label.width + padX * 2, label.height + padY * 2, 6)
        .fill({ color: this.palette.BG_RAISE, alpha: 0.85 })
        .stroke({ width: 1, color: this.palette.LINE });
      label.position.set(10 + padX, 10 + padY);
      container.addChild(backdrop, label);
    }

    container.eventMode = "static";
    container.cursor = "pointer";
    container.hitArea = new Rectangle(0, 0, node.width, node.height);
    container.on("pointerdown", (e: FederatedPointerEvent) => {
      if (this.linking) return;
      e.stopPropagation();
      this.beginNodeDrag(node.id, e);
    });
    container.on("pointertap", () => {
      if (this.suppressTapFor === node.id) {
        this.suppressTapFor = null;
        return;
      }
      this.selectNode(node.id);
    });
    container.on("pointerover", () => this.setHoveredNode(node.id));
    container.on("pointerout", () => {
      if (this.hoveredId === node.id) this.setHoveredNode(null);
    });

    return { container, bg, node };
  }

  private paintGroup(bg: Graphics, node: CanvasNode, selected: boolean, hovered: boolean) {
    const accent = this.resolveNodeColor(node.color);
    const fill = (accent ? hexToRgba(accent, 0.06) : null) ?? this.palette.GROUP_WASH;
    bg.clear().roundRect(0, 0, node.width, node.height, CARD_RADIUS + 4).fill(fill);
    if (hovered && !selected) {
      bg.roundRect(0, 0, node.width, node.height, CARD_RADIUS + 4).fill(this.palette.GROUP_WASH);
    }
    bg.roundRect(0, 0, node.width, node.height, CARD_RADIUS + 4).stroke({
      width: selected ? 1.5 : 1,
      color: selected ? this.palette.AMBER : accent ?? this.palette.LINE_STRONG,
    });
    if (selected) {
      bg.roundRect(-3, -3, node.width + 6, node.height + 6, CARD_RADIUS + 7).stroke({
        width: 2,
        color: this.palette.ACCENT_GLOW,
      });
    }
  }

  /** Cards whose centers sit inside the group's current bounds ride along with it. */
  private groupMemberCards(group: GroupCard): Array<{ id: string; card: NodeCard }> {
    const gx = group.container.x;
    const gy = group.container.y;
    const { width, height } = group.node;
    const members: Array<{ id: string; card: NodeCard }> = [];
    for (const [id, card] of this.nodeCards) {
      const cx = card.container.x + card.node.width / 2;
      const cy = card.container.y + card.node.height / 2;
      if (cx >= gx && cx <= gx + width && cy >= gy && cy <= gy + height) {
        members.push({ id, card });
      }
    }
    return members;
  }

  private buildCard(node: CanvasNode): NodeCard {
    const container = new Container();
    container.position.set(node.x, node.y);

    const bg = new Graphics();
    container.addChild(bg);
    this.paintCard(bg, node, false);

    const accent = this.resolveNodeColor(node.color);
    if (accent) {
      const stripe = new Graphics().roundRect(0, 0, 4, node.height, 2).fill(accent);
      container.addChild(stripe);
    }

    const textX = CARD_PADDING + (accent ? 4 : 0);
    const textWidth = Math.max(8, node.width - textX - CARD_PADDING);

    if (node.type === "file") {
      const kind = classifyPath(node.file);
      const hue = this.palette.KIND[kind];
      container.addChild(this.buildKindChip(kind, hue, textX, CARD_PADDING));

      const headX = textX + CHIP_SIZE + 9;
      const headWidth = Math.max(8, node.width - headX - CARD_PADDING);
      const title = new Text({
        text: basename(node.file),
        style: {
          fontFamily: this.palette.FONT_UI,
          fontSize: 13,
          fontWeight: "600",
          fill: this.palette.TEXT,
          wordWrap: true,
          wordWrapWidth: headWidth,
          breakWords: true,
        },
      });
      title.position.set(headX, CARD_PADDING + 1);
      container.addChild(title);

      const kindLabel = new Text({
        text: KIND_LABELS[kind].toUpperCase(),
        style: {
          fontFamily: this.palette.FONT_MONO,
          fontSize: 9.5,
          letterSpacing: 0.8,
          fill: hue,
        },
      });
      kindLabel.position.set(headX, CARD_PADDING + 1 + title.height + 4);
      container.addChild(kindLabel);
    } else if (node.type === "link") {
      const hue = this.palette.KIND.file;
      container.addChild(this.buildLinkChip(hue, textX, CARD_PADDING));

      const headX = textX + CHIP_SIZE + 9;
      const caption = new Text({
        text: "LINK",
        style: {
          fontFamily: this.palette.FONT_MONO,
          fontSize: 9.5,
          letterSpacing: 0.8,
          fill: this.palette.FAINT,
        },
      });
      caption.position.set(headX, CARD_PADDING + 2);
      container.addChild(caption);

      const url = new Text({
        text: node.url,
        style: {
          fontFamily: this.palette.FONT_MONO,
          fontSize: 11.5,
          lineHeight: 16,
          fill: this.palette.MUTED,
          wordWrap: true,
          wordWrapWidth: Math.max(8, node.width - headX - CARD_PADDING),
          breakWords: true,
        },
      });
      url.position.set(headX, CARD_PADDING + 2 + caption.height + 4);
      container.addChild(url);
    } else if (node.type === "text") {
      const body = node.text.length > 300 ? `${node.text.slice(0, 300)}…` : node.text;
      const bodyText = new Text({
        text: body,
        style: {
          fontFamily: this.palette.FONT_UI,
          fontSize: 12.5,
          lineHeight: 19,
          fill: this.palette.TEXT_SOFT,
          wordWrap: true,
          wordWrapWidth: Math.max(8, textWidth - 4),
          breakWords: true,
        },
      });
      bodyText.position.set(textX + 2, CARD_PADDING + 2);
      container.addChild(bodyText);
    }

    const ports = new Map<Side, Graphics>();
    for (const side of SIDES) {
      const port = this.buildPort(node, side);
      container.addChild(port);
      ports.set(side, port);
    }

    const resizeHandle = this.buildResizeHandle(node);
    container.addChild(resizeHandle);

    container.eventMode = "static";
    container.cursor = "pointer";
    container.hitArea = new Rectangle(0, 0, node.width, node.height);
    container.on("pointerdown", (e: FederatedPointerEvent) => {
      if (this.linking) return;
      e.stopPropagation();
      this.beginNodeDrag(node.id, e);
    });
    container.on("pointertap", () => this.onNodeTap(node));
    container.on("pointerover", () => this.setHoveredNode(node.id));
    container.on("pointerout", () => {
      if (this.hoveredId === node.id) this.setHoveredNode(null);
    });

    return { container, bg, node, ports, resizeHandle };
  }

  /** Rounded chip with a per-kind wash + Graphics glyph (no React/DOM in Pixi). */
  private buildKindChip(kind: ResourceKind, hue: string, x: number, y: number): Graphics {
    const chip = new Graphics()
      .roundRect(0, 0, CHIP_SIZE, CHIP_SIZE, CHIP_RADIUS)
      .fill(hexToRgba(hue, 0.14) ?? this.palette.AMBER_WASH)
      .stroke({ width: 1, color: hexToRgba(hue, 0.4) ?? hue });
    drawKindGlyph(chip, kind, CHIP_SIZE / 2, CHIP_SIZE / 2, 14, hue);
    chip.position.set(x, y);
    return chip;
  }

  private buildLinkChip(hue: string, x: number, y: number): Graphics {
    const chip = new Graphics()
      .roundRect(0, 0, CHIP_SIZE, CHIP_SIZE, CHIP_RADIUS)
      .fill(hexToRgba(hue, 0.14) ?? this.palette.AMBER_WASH)
      .stroke({ width: 1, color: hexToRgba(hue, 0.4) ?? hue });
    drawLinkGlyph(chip, CHIP_SIZE / 2, CHIP_SIZE / 2, 14, hue);
    chip.position.set(x, y);
    return chip;
  }

  /** Hover repaints touch only the two affected cards, not every node. */
  private setHoveredNode(id: string | null) {
    if (this.hoveredId === id) return;
    const prev = this.hoveredId;
    this.hoveredId = id;
    if (prev) this.refreshCardChrome(prev);
    if (id) this.refreshCardChrome(id);
  }

  /** Re-derive one card's paint/ports/handle from current selection+hover state. */
  private refreshCardChrome(id: string) {
    const card = this.nodeCards.get(id);
    if (card) {
      const selected = this.selectedId === id;
      this.paintCard(card.bg, card.node, selected, this.hoveredId === id);
      card.resizeHandle.visible = selected;
      this.updateCardPorts(id, card);
      return;
    }
    const group = this.groupCards.get(id);
    if (group) this.paintGroup(group.bg, group.node, this.selectedId === id, this.hoveredId === id);
  }

  private buildResizeHandle(node: CanvasNode): Graphics {
    const handle = new Graphics();
    handle.eventMode = "static";
    handle.cursor = "nwse-resize";
    handle.visible = false;
    handle.hitArea = new Rectangle(-2, -2, RESIZE_HANDLE + 4, RESIZE_HANDLE + 4);
    this.layoutResizeHandle(handle, node);
    this.paintResizeHandle(handle);
    handle.on("pointerdown", (e: FederatedPointerEvent) => {
      e.stopPropagation();
      this.beginResize(node.id, e);
    });
    return handle;
  }

  private layoutResizeHandle(handle: Graphics, node: CanvasNode) {
    handle.position.set(node.width - RESIZE_HANDLE, node.height - RESIZE_HANDLE);
  }

  private paintResizeHandle(handle: Graphics) {
    handle
      .clear()
      .roundRect(0, 0, RESIZE_HANDLE, RESIZE_HANDLE, 2)
      .fill(this.palette.AMBER)
      .stroke({ width: 1, color: this.palette.AMBER_BRIGHT });
  }

  private buildPort(node: CanvasNode, side: Side): Graphics {
    const port = new Graphics();
    const local = portLocal(node, side);
    port.position.set(local.x, local.y);
    port.eventMode = "static";
    port.cursor = "crosshair";
    port.hitArea = new Rectangle(-PORT_HIT, -PORT_HIT, PORT_HIT * 2, PORT_HIT * 2);
    // Hidden until the node is hovered/selected or a link is in flight.
    port.visible = false;
    this.paintPort(port, false);
    port.on("pointerdown", (e: FederatedPointerEvent) => {
      e.stopPropagation();
      this.beginLink(node.id, side, e);
    });
    return port;
  }

  private paintPort(port: Graphics, active: boolean) {
    port.clear()
      .circle(0, 0, PORT_RADIUS)
      .fill(active ? this.palette.AMBER : this.palette.PANEL)
      .stroke({ width: 1.5, color: active ? this.palette.AMBER_BRIGHT : this.palette.AMBER });
  }

  /** Ports show while linking (targets), or on the hovered/selected node. */
  private updateCardPorts(id: string, card: NodeCard) {
    const direct = this.selectedId === id || this.hoveredId === id;
    const show = this.linking !== null || direct;
    for (const [side, port] of card.ports) {
      const isSource = this.linking?.fromId === id && this.linking.fromSide === side;
      port.visible = show;
      port.alpha = isSource || direct ? 1 : 0.7;
      this.paintPort(port, isSource);
    }
  }

  private refreshPortVisibility() {
    for (const [id, card] of this.nodeCards) {
      this.updateCardPorts(id, card);
    }
  }

  private paintCard(bg: Graphics, node: CanvasNode, selected: boolean, hovered = false) {
    const { width, height } = node;
    const accent = this.resolveNodeColor(node.color);
    const fill =
      node.type === "text"
        ? (hexToRgba(this.palette.AMBER, 0.14) ?? this.palette.AMBER_WASH)
        : this.palette.PANEL;
    bg.clear();
    // Painted elevation: two offset layers stand in for a blur (no Pixi filters).
    bg.roundRect(-1, 3, width + 2, height + 1, CARD_RADIUS + 2).fill(this.palette.SHADOW_SOFT);
    bg.roundRect(0, 1.5, width, height + 0.5, CARD_RADIUS + 1).fill(this.palette.SHADOW);
    if (selected) {
      bg.roundRect(-3, -3, width + 6, height + 6, CARD_RADIUS + 3).stroke({
        width: 2,
        color: this.palette.ACCENT_GLOW,
      });
    }
    bg.roundRect(0, 0, width, height, CARD_RADIUS).fill(fill);
    if (accent) {
      const tint = hexToRgba(accent, 0.06);
      if (tint) bg.roundRect(0, 0, width, height, CARD_RADIUS).fill(tint);
    }
    if (hovered && !selected) {
      bg.roundRect(0, 0, width, height, CARD_RADIUS).fill(this.palette.HOVER);
    }
    bg.roundRect(0, 0, width, height, CARD_RADIUS).stroke({
      width: selected ? 1.5 : 1,
      color: selected
        ? this.palette.AMBER
        : hovered
          ? this.palette.LINE_STRONG
          : this.palette.BORDER,
    });
  }

  private drawEdge(
    g: Graphics,
    shell: Container,
    edge: CanvasEdge,
    from: CanvasNode,
    to: CanvasNode,
    selected: boolean,
    hovered = false,
  ) {
    const fromSide = edge.fromSide ?? autoSide(from, to);
    const toSide = edge.toSide ?? autoSide(to, from);
    const start = sidePoint(from, fromSide);
    const end = sidePoint(to, toSide);

    const dist = Math.hypot(end.x - start.x, end.y - start.y);
    const bend = Math.min(90, Math.max(24, dist * 0.35));
    const n1 = SIDE_NORMAL[fromSide];
    const n2 = SIDE_NORMAL[toSide];
    const cp1 = { x: start.x + n1.x * bend, y: start.y + n1.y * bend };
    const cp2 = { x: end.x + n2.x * bend, y: end.y + n2.y * bend };

    const accent = this.resolveNodeColor(edge.color);
    const stroke = selected
      ? this.palette.AMBER
      : accent ?? (hovered ? this.palette.MUTED : this.palette.LINE_STRONG);
    g.clear();
    // Wide near-invisible stroke for easier hit testing (and edge hover).
    g.moveTo(start.x, start.y).bezierCurveTo(cp1.x, cp1.y, cp2.x, cp2.y, end.x, end.y).stroke({
      width: 14,
      color: 0xffffff,
      alpha: 0.001,
    });
    g.moveTo(start.x, start.y).bezierCurveTo(cp1.x, cp1.y, cp2.x, cp2.y, end.x, end.y).stroke({
      width: selected ? 2.5 : hovered ? 2 : 1.5,
      color: stroke,
    });

    // Arrowhead pointing into the target node (opposite its outward normal).
    const dir = { x: -n2.x, y: -n2.y };
    const size = 6;
    const perp = { x: -dir.y, y: dir.x };
    const tip = end;
    const left = { x: tip.x - dir.x * size + perp.x * (size * 0.45), y: tip.y - dir.y * size + perp.y * (size * 0.45) };
    const right = { x: tip.x - dir.x * size - perp.x * (size * 0.45), y: tip.y - dir.y * size - perp.y * (size * 0.45) };
    g.poly([tip.x, tip.y, left.x, left.y, right.x, right.y]).fill(stroke);

    // Drop previous label children (everything except the path graphics at index 0).
    while (shell.children.length > 1) {
      shell.removeChildAt(1).destroy();
    }
    if (edge.label) {
      const mid = bezierPoint(start, cp1, cp2, end, 0.5);
      const label = new Text({
        text: edge.label,
        style: {
          fontFamily: this.palette.FONT_UI,
          fontSize: 11,
          fill: this.palette.MUTED,
        },
      });
      label.anchor.set(0.5);
      label.position.set(mid.x, mid.y);
      const pad = 4;
      const backdrop = new Graphics()
        .roundRect(
          mid.x - label.width / 2 - pad,
          mid.y - label.height / 2 - pad / 2,
          label.width + pad * 2,
          label.height + pad,
          4,
        )
        .fill(this.palette.PANEL)
        .stroke({ width: 1, color: this.palette.LINE });
      shell.addChild(backdrop, label);
    }
  }

  private onNodeTap(node: CanvasNode) {
    if (this.suppressTapFor === node.id) {
      this.suppressTapFor = null;
      return;
    }
    this.selectNode(node.id);
    const now = performance.now();
    const last = this.lastTapAt.get(node.id) ?? 0;
    this.lastTapAt.set(node.id, now);
    if (now - last >= DOUBLE_CLICK_MS) return;
    this.lastTapAt.delete(node.id);
    if (node.type === "file") {
      this.options.onOpenFile(node.file, node.subpath);
    } else if (node.type === "text") {
      this.options.onEditText?.(node.id, node.text);
    }
  }

  selectNode(id: string | null) {
    if (this.selectedEdgeId) {
      this.clearEdgeSelection();
    }
    if (this.selectedId === id) {
      this.refreshSelectionChrome();
      return;
    }
    const prev = this.selectedId;
    this.selectedId = id;
    if (prev) this.refreshCardChrome(prev);
    if (id) this.refreshCardChrome(id);
    this.options.onSelectNode?.(id);
  }

  selectEdge(id: string | null) {
    if (this.selectedId) {
      const prev = this.selectedId;
      this.selectedId = null;
      this.refreshCardChrome(prev);
      this.options.onSelectNode?.(null);
    }
    if (this.selectedEdgeId === id) {
      this.refreshEdgeSelection();
      return;
    }
    this.selectedEdgeId = id;
    this.refreshEdgeSelection();
    this.options.onSelectEdge?.(id);
  }

  private clearEdgeSelection() {
    this.selectedEdgeId = null;
    this.refreshEdgeSelection();
    this.options.onSelectEdge?.(null);
  }

  private refreshEdgeSelection() {
    if (!this.data) return;
    const byId = new Map(this.data.nodes.map((n) => [n.id, n]));
    for (const edge of this.data.edges) {
      const g = this.edgeGraphics.get(edge.id);
      const from = byId.get(edge.fromNode);
      const to = byId.get(edge.toNode);
      if (!g || !from || !to || !g.parent) continue;
      this.drawEdge(
        g,
        g.parent as Container,
        edge,
        from,
        to,
        this.selectedEdgeId === edge.id,
        this.hoveredEdgeId === edge.id,
      );
    }
  }

  private refreshSelectionChrome() {
    if (this.selectedId) this.refreshCardChrome(this.selectedId);
  }

  moveSelectedBy(dx: number, dy: number): boolean {
    if (!this.selectedId) return false;
    const card = this.nodeCards.get(this.selectedId);
    if (card) {
      const x = card.node.x + dx;
      const y = card.node.y + dy;
      card.node = { ...card.node, x, y };
      card.container.position.set(x, y);
      this.options.onMoveNodes?.([{ id: this.selectedId, x, y }]);
      return true;
    }
    const group = this.groupCards.get(this.selectedId);
    if (!group) return false;
    const moves: CanvasNodeMove[] = [];
    for (const member of this.groupMemberCards(group)) {
      const mx = member.card.node.x + dx;
      const my = member.card.node.y + dy;
      member.card.node = { ...member.card.node, x: mx, y: my };
      member.card.container.position.set(mx, my);
      moves.push({ id: member.id, x: mx, y: my });
    }
    const x = group.node.x + dx;
    const y = group.node.y + dy;
    group.node = { ...group.node, x, y };
    group.container.position.set(x, y);
    moves.push({ id: this.selectedId, x, y });
    this.options.onMoveNodes?.(moves);
    return true;
  }

  removeSelected(): boolean {
    if (this.selectedEdgeId) {
      const id = this.selectedEdgeId;
      this.options.onRemoveEdges?.([id]);
      return true;
    }
    if (!this.selectedId) return false;
    const id = this.selectedId;
    this.options.onRemoveNodes?.([id]);
    return true;
  }

  private beginNodeDrag(id: string, event: FederatedPointerEvent) {
    const card = this.nodeCards.get(id);
    const group = card ? undefined : this.groupCards.get(id);
    if (!card && !group) return;
    this.selectNode(id);
    const container = card ? card.container : group!.container;
    // Group drags carry every card whose center sits inside the group frame.
    const members = group
      ? this.groupMemberCards(group).map((member) => ({
          id: member.id,
          container: member.card.container,
          originX: member.card.container.x,
          originY: member.card.container.y,
        }))
      : [];
    // Prefer the live container pose — card.node can lag if a prior drag
    // committed visually before React/disk state caught up.
    this.dragging = {
      id,
      container,
      startX: event.global.x,
      startY: event.global.y,
      nodeX: container.x,
      nodeY: container.y,
      members,
      moved: false,
    };
  }

  private beginResize(id: string, event: FederatedPointerEvent) {
    const card = this.nodeCards.get(id);
    if (!card) return;
    this.selectNode(id);
    this.resizing = {
      id,
      startX: event.global.x,
      startY: event.global.y,
      width: card.node.width,
      height: card.node.height,
      moved: false,
    };
  }

  private applyLiveSize(card: NodeCard, width: number, height: number) {
    card.node = { ...card.node, width, height };
    card.container.hitArea = new Rectangle(0, 0, width, height);
    this.paintCard(card.bg, card.node, this.selectedId === card.node.id);
    this.layoutResizeHandle(card.resizeHandle, card.node);
    for (const [side, port] of card.ports) {
      const local = portLocal(card.node, side);
      port.position.set(local.x, local.y);
    }
  }

  private beginLink(fromId: string, fromSide: Side, event: FederatedPointerEvent) {
    this.cancelLink();
    this.selectNode(fromId);
    const preview = new Graphics();
    this.world.addChild(preview);
    this.linking = { fromId, fromSide, preview };
    this.refreshPortVisibility();
    this.updateLinkPreview(event.global.x, event.global.y);
  }

  private cancelLink() {
    if (!this.linking) return;
    this.world.removeChild(this.linking.preview);
    this.linking.preview.destroy();
    this.linking = null;
    this.refreshPortVisibility();
  }

  private updateLinkPreview(globalX: number, globalY: number) {
    const link = this.linking;
    if (!link) return;
    const fromCard = this.nodeCards.get(link.fromId);
    if (!fromCard) return;
    const start = {
      x: fromCard.container.x + portLocal(fromCard.node, link.fromSide).x,
      y: fromCard.container.y + portLocal(fromCard.node, link.fromSide).y,
    };
    const end = this.toWorld(globalX, globalY);
    const n1 = SIDE_NORMAL[link.fromSide];
    const bend = Math.min(90, Math.max(24, Math.hypot(end.x - start.x, end.y - start.y) * 0.35));
    link.preview
      .clear()
      .moveTo(start.x, start.y)
      .bezierCurveTo(
        start.x + n1.x * bend,
        start.y + n1.y * bend,
        end.x,
        end.y,
        end.x,
        end.y,
      )
      .stroke({ width: 1.5, color: this.palette.AMBER, alpha: 0.85 });
  }

  private finishLink(globalX: number, globalY: number) {
    const link = this.linking;
    if (!link) return;
    const target = this.hitTestPort(globalX, globalY)
      ?? this.hitTestNodeSide(globalX, globalY);
    this.cancelLink();
    if (!target || target.id === link.fromId) return;
    this.options.onConnectNodes?.({
      fromNode: link.fromId,
      toNode: target.id,
      fromSide: link.fromSide,
      toSide: target.side,
    });
  }

  private toWorld(globalX: number, globalY: number): { x: number; y: number } {
    return {
      x: (globalX - this.world.position.x) / this.world.scale.x,
      y: (globalY - this.world.position.y) / this.world.scale.y,
    };
  }

  private hitTestPort(globalX: number, globalY: number): { id: string; side: Side } | null {
    const world = this.toWorld(globalX, globalY);
    for (const [id, card] of this.nodeCards) {
      for (const side of SIDES) {
        const local = portLocal(card.node, side);
        const px = card.container.x + local.x;
        const py = card.container.y + local.y;
        if (Math.hypot(world.x - px, world.y - py) <= PORT_HIT + 2) {
          return { id, side };
        }
      }
    }
    return null;
  }

  private hitTestNodeSide(globalX: number, globalY: number): { id: string; side: Side } | null {
    const world = this.toWorld(globalX, globalY);
    for (const [id, card] of this.nodeCards) {
      const { x, y } = card.container.position;
      const { width, height } = card.node;
      if (world.x < x || world.y < y || world.x > x + width || world.y > y + height) continue;
      const live: CanvasNode = { ...card.node, x, y };
      const fromCard = this.linking ? this.nodeCards.get(this.linking.fromId) : undefined;
      if (!fromCard || !this.linking) return { id, side: "left" };
      const fromLive: CanvasNode = {
        ...fromCard.node,
        x: fromCard.container.x,
        y: fromCard.container.y,
      };
      return { id, side: autoSide(live, fromLive) };
    }
    return null;
  }

  private fitToContent(nodes: CanvasNode[]) {
    if (nodes.length === 0) return;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const n of nodes) {
      minX = Math.min(minX, n.x);
      minY = Math.min(minY, n.y);
      maxX = Math.max(maxX, n.x + n.width);
      maxY = Math.max(maxY, n.y + n.height);
    }
    const camera = this.cameraForBounds({
      x: minX,
      y: minY,
      width: Math.max(1, maxX - minX),
      height: Math.max(1, maxY - minY),
    });
    this.applyCamera(camera);
  }

  private cameraForBounds(bounds: { x: number; y: number; width: number; height: number }): {
    x: number;
    y: number;
    scale: number;
  } {
    const boxW = Math.max(1, bounds.width);
    const boxH = Math.max(1, bounds.height);
    const screenW = this.app.screen.width || this.host.clientWidth || 800;
    const screenH = this.app.screen.height || this.host.clientHeight || 600;
    const scale = clamp(Math.min(screenW / boxW, screenH / boxH) * 0.88, MIN_SCALE, MAX_SCALE);
    return {
      scale,
      x: screenW / 2 - (bounds.x + boxW / 2) * scale,
      y: screenH / 2 - (bounds.y + boxH / 2) * scale,
    };
  }

  private applyCamera(camera: { x: number; y: number; scale: number }) {
    this.world.scale.set(camera.scale);
    this.world.position.set(camera.x, camera.y);
    this.syncCamera();
  }

  private animateCameraTo(
    target: { x: number; y: number; scale: number },
    durationMs: number,
  ): Promise<void> {
    this.cancelCameraTween();
    if (durationMs <= 0 || !Number.isFinite(durationMs)) {
      this.applyCamera(target);
      return Promise.resolve();
    }
    const from = {
      x: this.world.position.x,
      y: this.world.position.y,
      scale: this.world.scale.x,
    };
    const token = ++this.cameraTweenToken;
    const started = performance.now();
    return new Promise((resolve) => {
      const step = (now: number) => {
        if (this.destroyed || this.cameraTweenToken !== token) {
          resolve();
          return;
        }
        const t = clamp((now - started) / durationMs, 0, 1);
        const e = easeInOutCubic(t);
        this.applyCamera({
          x: from.x + (target.x - from.x) * e,
          y: from.y + (target.y - from.y) * e,
          scale: from.scale + (target.scale - from.scale) * e,
        });
        if (t >= 1) {
          this.cameraTween = null;
          resolve();
          return;
        }
        this.cameraTween = { token, raf: requestAnimationFrame(step) };
      };
      this.cameraTween = { token, raf: requestAnimationFrame(step) };
    });
  }

  private onStagePointerDown = (e: FederatedPointerEvent) => {
    if (this.linking) {
      // Click empty space while linking cancels; otherwise ports handle their own down.
      if (e.target === this.app.stage) this.cancelLink();
      return;
    }
    if (e.target !== this.app.stage) return;
    this.selectNode(null);
    this.selectEdge(null);
    this.cancelCameraTween();
    this.panning = true;
    this.panStart = { x: e.global.x, y: e.global.y };
    this.panOrigin = { x: this.world.position.x, y: this.world.position.y };
  };

  private onStagePointerMove = (e: FederatedPointerEvent) => {
    if (this.linking) {
      this.updateLinkPreview(e.global.x, e.global.y);
      return;
    }
    if (this.resizing) {
      const resize = this.resizing;
      const card = this.nodeCards.get(resize.id);
      if (!card) return;
      const dx = (e.global.x - resize.startX) / this.world.scale.x;
      const dy = (e.global.y - resize.startY) / this.world.scale.y;
      if (Math.abs(dx) > 1 || Math.abs(dy) > 1) resize.moved = true;
      const width = Math.max(MIN_NODE_SIZE, resize.width + dx);
      const height = Math.max(MIN_NODE_SIZE, resize.height + dy);
      this.applyLiveSize(card, width, height);
      return;
    }
    if (this.dragging) {
      const drag = this.dragging;
      const dx = (e.global.x - drag.startX) / this.world.scale.x;
      const dy = (e.global.y - drag.startY) / this.world.scale.y;
      if (Math.abs(dx) > 1 || Math.abs(dy) > 1) drag.moved = true;
      drag.container.position.set(drag.nodeX + dx, drag.nodeY + dy);
      for (const member of drag.members) {
        member.container.position.set(member.originX + dx, member.originY + dy);
      }
      return;
    }
    if (!this.panning) return;
    const dx = e.global.x - this.panStart.x;
    const dy = e.global.y - this.panStart.y;
    this.world.position.set(this.panOrigin.x + dx, this.panOrigin.y + dy);
    this.syncCamera();
  };

  private onStagePointerUp = (e: FederatedPointerEvent) => {
    if (this.linking) {
      this.finishLink(e.global.x, e.global.y);
      return;
    }
    if (this.resizing) {
      const resize = this.resizing;
      const card = this.nodeCards.get(resize.id);
      this.resizing = null;
      if (card && resize.moved) {
        this.suppressTapFor = resize.id;
        this.options.onResizeNodes?.([{
          id: resize.id,
          width: card.node.width,
          height: card.node.height,
        }]);
      }
      return;
    }
    if (this.dragging) {
      const drag = this.dragging;
      this.dragging = null;
      if (drag.moved) {
        this.suppressTapFor = drag.id;
        const moves: CanvasNodeMove[] = [];
        const card = this.nodeCards.get(drag.id);
        const group = card ? undefined : this.groupCards.get(drag.id);
        if (card) {
          card.node = { ...card.node, x: card.container.x, y: card.container.y };
          moves.push({ id: drag.id, x: card.container.x, y: card.container.y });
        } else if (group) {
          group.node = { ...group.node, x: group.container.x, y: group.container.y };
          moves.push({ id: drag.id, x: group.container.x, y: group.container.y });
        }
        for (const member of drag.members) {
          const memberCard = this.nodeCards.get(member.id);
          if (!memberCard) continue;
          memberCard.node = {
            ...memberCard.node,
            x: member.container.x,
            y: member.container.y,
          };
          moves.push({ id: member.id, x: member.container.x, y: member.container.y });
        }
        if (moves.length > 0) this.options.onMoveNodes?.(moves);
      }
    }
    this.panning = false;
  };

  private onWheel = (e: WheelEvent) => {
    e.preventDefault();
    this.cancelCameraTween();
    const rect = this.app.canvas.getBoundingClientRect();
    const cursorX = e.clientX - rect.left;
    const cursorY = e.clientY - rect.top;

    if (e.ctrlKey || e.metaKey) {
      const factor = Math.exp(-e.deltaY * 0.01);
      const oldScale = this.world.scale.x;
      const newScale = clamp(oldScale * factor, MIN_SCALE, MAX_SCALE);
      if (newScale === oldScale) return;
      const worldX = (cursorX - this.world.position.x) / oldScale;
      const worldY = (cursorY - this.world.position.y) / oldScale;
      this.world.scale.set(newScale);
      this.world.position.set(cursorX - worldX * newScale, cursorY - worldY * newScale);
    } else {
      this.world.position.set(this.world.position.x - e.deltaX, this.world.position.y - e.deltaY);
    }
    this.syncCamera();
  };

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.cancelCameraTween();
    this.cancelLink();
    this.zoomListeners.clear();
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    this.disconnectThemeObserver?.();
    this.disconnectThemeObserver = null;
    // Before init resolves, app.canvas/stage don't exist yet; the ready
    // handler above notices `destroyed` and finishes the teardown itself.
    if (!this.initialized) return;
    this.app.canvas.removeEventListener("wheel", this.onWheel);
    this.app.stage.off("pointerdown", this.onStagePointerDown);
    this.app.stage.off("globalpointermove", this.onStagePointerMove);
    this.app.stage.off("pointerup", this.onStagePointerUp);
    this.app.stage.off("pointerupoutside", this.onStagePointerUp);
    this.app.destroy(true, { children: true, texture: true });
  }
}

function portLocal(node: Pick<CanvasNode, "width" | "height">, side: Side): { x: number; y: number } {
  switch (side) {
    case "top":
      return { x: node.width / 2, y: 0 };
    case "bottom":
      return { x: node.width / 2, y: node.height };
    case "left":
      return { x: 0, y: node.height / 2 };
    case "right":
      return { x: node.width, y: node.height / 2 };
  }
}

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

function easeInOutCubic(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
}

/**
 * Kind glyphs on KindMark's 20-unit lattice grid, painted with Graphics
 * primitives (no React/SVG in the Pixi scene). Centered at (cx, cy), sized
 * to `size` px, stroked/filled in the kind hue.
 */
function drawKindGlyph(
  g: Graphics,
  kind: ResourceKind,
  cx: number,
  cy: number,
  size: number,
  color: string,
) {
  const u = size / 20;
  const px = (x: number) => cx + (x - 10) * u;
  const py = (y: number) => cy + (y - 10) * u;
  const stroke = { width: 1.4, color, cap: "round" as const, join: "round" as const };

  switch (kind) {
    case "page":
      // Doc lines, the last one trailing off.
      g.moveTo(px(4), py(5.5)).lineTo(px(16), py(5.5))
        .moveTo(px(4), py(10)).lineTo(px(16), py(10))
        .moveTo(px(4), py(14.5)).lineTo(px(11), py(14.5))
        .stroke(stroke);
      break;
    case "canvas":
      // Spatial frame with corner nodes.
      g.rect(px(5), py(5), 10 * u, 10 * u).stroke(stroke);
      for (const [x, y] of [[5, 5], [15, 5], [5, 15], [15, 15]] as const) {
        g.circle(px(x), py(y), 1.4 * u).fill(color);
      }
      break;
    case "data-app":
      // Grid of typed records.
      for (const x of [5, 10, 15]) {
        for (const y of [5, 10, 15]) {
          g.circle(px(x), py(y), 1.4 * u).fill(color);
        }
      }
      break;
    case "dataset":
      // Bars off the baseline.
      g.moveTo(px(6), py(16)).lineTo(px(6), py(11))
        .moveTo(px(10), py(16)).lineTo(px(10), py(6))
        .moveTo(px(14), py(16)).lineTo(px(14), py(9))
        .stroke({ ...stroke, width: 2.2 });
      break;
    case "notebook":
      // Input cell over output cell.
      g.roundRect(px(4.5), py(4.5), 11 * u, 11 * u, 1.5 * u).stroke(stroke);
      g.moveTo(px(4.5), py(10)).lineTo(px(15.5), py(10)).stroke(stroke);
      g.circle(px(7), py(7.25), 1.2 * u).fill(color);
      break;
    case "ink":
      // A drawn stroke.
      g.moveTo(px(4), py(14))
        .bezierCurveTo(px(7), py(8), px(9), py(18), px(12), py(12))
        .quadraticCurveTo(px(14), py(8.6), px(16), py(8.5))
        .stroke(stroke);
      break;
    case "artifact":
      // Sealed box.
      g.roundRect(px(5), py(6), 10 * u, 8.5 * u, 1 * u).stroke(stroke);
      g.moveTo(px(10), py(6)).lineTo(px(10), py(14.5)).stroke(stroke);
      g.moveTo(px(5), py(9)).lineTo(px(15), py(9)).stroke(stroke);
      break;
    case "app":
      // Window with a title bar.
      g.roundRect(px(4.5), py(4.5), 11 * u, 11 * u, 2 * u).stroke(stroke);
      g.moveTo(px(4.5), py(8)).lineTo(px(15.5), py(8)).stroke(stroke);
      g.circle(px(6.8), py(6.3), 0.9 * u).fill(color);
      break;
    case "workflow":
      // Arrow chain: start node, path, arrowhead.
      g.circle(px(5), py(15), 1.5 * u).fill(color);
      g.moveTo(px(6.5), py(13.5)).lineTo(px(14.2), py(5.8)).stroke(stroke);
      g.moveTo(px(10.2), py(5.4)).lineTo(px(14.6), py(5.4)).lineTo(px(14.6), py(9.8)).stroke(stroke);
      break;
    case "task":
      // Node plus completion tick.
      g.circle(px(10), py(10), 6 * u).stroke(stroke);
      g.moveTo(px(7.5), py(10)).lineTo(px(9.3), py(11.8)).lineTo(px(13), py(8.2)).stroke(stroke);
      break;
    case "derived":
      // Generated output: a diamond.
      g.poly([px(10), py(4.5), px(15.5), py(10), px(10), py(15.5), px(4.5), py(10)]).stroke(stroke);
      break;
    case "folder":
      g.moveTo(px(3.5), py(7.5)).lineTo(px(7.5), py(7.5)).lineTo(px(9), py(5.5))
        .lineTo(px(16.5), py(5.5)).lineTo(px(16.5), py(16)).lineTo(px(3.5), py(16))
        .closePath()
        .stroke(stroke);
      break;
    default:
      // file — plain doc.
      g.roundRect(px(6), py(3.5), 8 * u, 13 * u, 1.5 * u).stroke(stroke);
      g.moveTo(px(8.5), py(8.5)).lineTo(px(13.5), py(8.5))
        .moveTo(px(8.5), py(11.5)).lineTo(px(13.5), py(11.5))
        .stroke(stroke);
      break;
  }
}

/** Chain-link glyph for URL nodes (link is not a ResourceKind). */
function drawLinkGlyph(g: Graphics, cx: number, cy: number, size: number, color: string) {
  const u = size / 20;
  const px = (x: number) => cx + (x - 10) * u;
  const py = (y: number) => cy + (y - 10) * u;
  const stroke = { width: 1.4, color, cap: "round" as const, join: "round" as const };
  g.circle(px(7), py(10), 3 * u).stroke(stroke);
  g.circle(px(13), py(10), 3 * u).stroke(stroke);
  g.moveTo(px(8.5), py(10)).lineTo(px(11.5), py(10)).stroke(stroke);
}

function bezierPoint(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number },
  t: number,
): { x: number; y: number } {
  const mt = 1 - t;
  const x = mt ** 3 * p0.x + 3 * mt ** 2 * t * p1.x + 3 * mt * t ** 2 * p2.x + t ** 3 * p3.x;
  const y = mt ** 3 * p0.y + 3 * mt ** 2 * t * p1.y + 3 * mt * t ** 2 * p2.y + t ** 3 * p3.y;
  return { x, y };
}
