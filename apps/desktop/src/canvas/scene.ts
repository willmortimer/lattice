import { Application, Container, FederatedPointerEvent, Graphics, Rectangle, Text } from "pixi.js";
import type { CanvasNodeMove, CanvasNodeSize } from "./adapter";
import type { CanvasData, CanvasEdge, CanvasNode } from "./types";
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

/** Parse a JSON Canvas node color: only hex is honored (Pixi cannot resolve CSS color-mix). */
function nodeAccent(color?: string): string | null {
  if (color && /^#[0-9a-fA-F]{3,8}$/.test(color)) return color;
  return null;
}

/**
 * Imperative PixiJS v8 scene for a read-only JSON Canvas view: pan, zoom,
 * node selection, and file-node double-click. No React, no editing.
 */
export class CanvasScene {
  private app = new Application();
  private world = new Container();
  private groupsLayer = new Container();
  private edgesLayer = new Container();
  private nodesLayer = new Container();

  private nodeCards = new Map<string, NodeCard>();
  private edgeGraphics = new Map<string, Graphics>();
  private selectedId: string | null = null;
  private selectedEdgeId: string | null = null;
  private hoveredId: string | null = null;
  private lastTapAt = new Map<string, number>();
  private suppressTapFor: string | null = null;
  private dragging: {
    id: string;
    startX: number;
    startY: number;
    nodeX: number;
    nodeY: number;
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

    this.world.addChild(this.groupsLayer, this.edgesLayer, this.nodesLayer);
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
      }
    });
    this.resizeObserver.observe(this.host);

    this.disconnectThemeObserver = observeThemeChange(() => {
      this.palette = readCanvasPalette();
      if (this.data) this.rebuild(this.data, { fit: false });
    });
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
      this.fitToContent(this.data.nodes);
    }
  }

  /** Convert a browser client point into canvas world coordinates (pan/zoom aware). */
  clientToWorld(clientX: number, clientY: number): { x: number; y: number } {
    const rect = this.app.canvas.getBoundingClientRect();
    return this.toWorld(clientX - rect.left, clientY - rect.top);
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
    this.edgeGraphics.clear();
    this.selectedId = null;
    this.selectedEdgeId = null;
    this.hoveredId = null;

    const byId = new Map(data.nodes.map((n) => [n.id, n]));

    for (const node of data.nodes) {
      if (node.type === "group") {
        this.groupsLayer.addChild(this.buildGroup(node));
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
      } else {
        this.pendingFit = false;
        this.needsInitialFit = false;
        this.fitToContent(data.nodes);
      }
    } else if (camera) {
      this.world.scale.set(camera.scale);
      this.world.position.set(camera.x, camera.y);
    }

    if (selectedId && this.nodeCards.has(selectedId)) {
      this.selectNode(selectedId);
    } else if (selectedEdgeId && this.edgeGraphics.has(selectedEdgeId)) {
      this.selectEdge(selectedEdgeId);
    }
  }

  private buildGroup(node: CanvasNode & { type: "group" }): Container {
    const container = new Container();
    container.position.set(node.x, node.y);

    const accent = nodeAccent(node.color);
    const bg = new Graphics()
      .roundRect(0, 0, node.width, node.height, CARD_RADIUS + 4)
      .fill(accent ? withAlpha(accent, 0.08) : this.palette.BG_RAISE)
      .stroke({ width: 1, color: accent ?? this.palette.LINE_STRONG });
    container.addChild(bg);

    if (node.label) {
      const label = new Text({
        text: node.label,
        style: {
          fontFamily: this.palette.FONT_DISPLAY,
          fontSize: 14,
          fontWeight: "600",
          fill: this.palette.TEXT_SOFT,
        },
      });
      label.position.set(2, -22);
      container.addChild(label);
    }

    container.eventMode = "static";
    container.cursor = "default";
    container.hitArea = new Rectangle(0, 0, node.width, node.height);
    container.on("pointerdown", (e: FederatedPointerEvent) => {
      e.stopPropagation();
      this.beginNodeDrag(node.id, e);
    });
    container.on("pointertap", () => this.selectNode(node.id));

    return container;
  }

  private buildCard(node: CanvasNode): NodeCard {
    const container = new Container();
    container.position.set(node.x, node.y);

    const bg = new Graphics();
    container.addChild(bg);
    this.paintCard(bg, node, false);

    const accent = nodeAccent(node.color);
    if (accent) {
      const stripe = new Graphics().roundRect(0, 0, 4, node.height, 2).fill(accent);
      container.addChild(stripe);
    }

    const textX = CARD_PADDING + (accent ? 4 : 0);
    const textWidth = Math.max(8, node.width - textX - CARD_PADDING);

    if (node.type === "file") {
      const kind = classifyPath(node.file);
      const title = new Text({
        text: basename(node.file),
        style: {
          fontFamily: this.palette.FONT_UI,
          fontSize: 13,
          fontWeight: "600",
          fill: this.palette.TEXT,
          wordWrap: true,
          wordWrapWidth: textWidth,
          breakWords: true,
        },
      });
      title.position.set(textX, CARD_PADDING);
      container.addChild(title);

      const kindLabel = new Text({
        text: KIND_LABELS[kind].toUpperCase(),
        style: {
          fontFamily: this.palette.FONT_MONO,
          fontSize: 10,
          letterSpacing: 0.6,
          fill: this.palette.AMBER_DEEP,
        },
      });
      kindLabel.position.set(textX, CARD_PADDING + title.height + 6);
      container.addChild(kindLabel);
    } else if (node.type === "link") {
      const title = new Text({
        text: "Link",
        style: {
          fontFamily: this.palette.FONT_MONO,
          fontSize: 10,
          letterSpacing: 0.6,
          fill: this.palette.FAINT,
        },
      });
      title.position.set(textX, CARD_PADDING);
      container.addChild(title);

      const url = new Text({
        text: node.url,
        style: {
          fontFamily: this.palette.FONT_MONO,
          fontSize: 12,
          fill: this.palette.AMBER,
          wordWrap: true,
          wordWrapWidth: textWidth,
          breakWords: true,
        },
      });
      url.position.set(textX, CARD_PADDING + title.height + 6);
      container.addChild(url);
    } else if (node.type === "text") {
      const body = node.text.length > 300 ? `${node.text.slice(0, 300)}…` : node.text;
      const bodyText = new Text({
        text: body,
        style: {
          fontFamily: this.palette.FONT_UI,
          fontSize: 12.5,
          lineHeight: 18,
          fill: this.palette.TEXT_SOFT,
          wordWrap: true,
          wordWrapWidth: textWidth,
          breakWords: true,
        },
      });
      bodyText.position.set(textX, CARD_PADDING);
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
    container.on("pointerover", () => {
      this.hoveredId = node.id;
      this.refreshPortVisibility();
    });
    container.on("pointerout", () => {
      if (this.hoveredId === node.id) this.hoveredId = null;
      this.refreshPortVisibility();
    });

    return { container, bg, node, ports, resizeHandle };
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
    port.alpha = 0.4;
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

  private refreshPortVisibility() {
    for (const [id, card] of this.nodeCards) {
      const emphasize =
        this.linking !== null
        || this.selectedId === id
        || this.hoveredId === id;
      for (const [side, port] of card.ports) {
        const isSource = this.linking?.fromId === id && this.linking.fromSide === side;
        port.alpha = isSource || emphasize ? 1 : 0.4;
        this.paintPort(port, isSource);
      }
    }
  }

  private paintCard(bg: Graphics, node: CanvasNode, selected: boolean) {
    const fill =
      node.type === "text"
        ? (hexToRgba(this.palette.AMBER, 0.14) ?? this.palette.AMBER_WASH)
        : this.palette.PANEL;
    bg
      .clear()
      .roundRect(0, 0, node.width, node.height, CARD_RADIUS)
      .fill(fill)
      .stroke({ width: selected ? 2 : 1, color: selected ? this.palette.AMBER : this.palette.BORDER });
  }

  private drawEdge(
    g: Graphics,
    shell: Container,
    edge: CanvasEdge,
    from: CanvasNode,
    to: CanvasNode,
    selected: boolean,
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

    const stroke = selected
      ? this.palette.AMBER
      : (nodeAccent(edge.color) ?? this.palette.LINE_STRONG);
    g.clear();
    // Wide near-invisible stroke for easier hit testing.
    g.moveTo(start.x, start.y).bezierCurveTo(cp1.x, cp1.y, cp2.x, cp2.y, end.x, end.y).stroke({
      width: 14,
      color: 0xffffff,
      alpha: 0.001,
    });
    g.moveTo(start.x, start.y).bezierCurveTo(cp1.x, cp1.y, cp2.x, cp2.y, end.x, end.y).stroke({
      width: selected ? 2.5 : 1.5,
      color: stroke,
    });

    // Arrowhead pointing into the target node (opposite its outward normal).
    const dir = { x: -n2.x, y: -n2.y };
    const size = 7;
    const perp = { x: -dir.y, y: dir.x };
    const tip = end;
    const left = { x: tip.x - dir.x * size + perp.x * (size * 0.55), y: tip.y - dir.y * size + perp.y * (size * 0.55) };
    const right = { x: tip.x - dir.x * size - perp.x * (size * 0.55), y: tip.y - dir.y * size - perp.y * (size * 0.55) };
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
        .fill(this.palette.PANEL);
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
    const prev = this.selectedId ? this.nodeCards.get(this.selectedId) : undefined;
    if (prev) {
      this.paintCard(prev.bg, prev.node, false);
      prev.resizeHandle.visible = false;
    }

    this.selectedId = id;
    const next = id ? this.nodeCards.get(id) : undefined;
    if (next) {
      this.paintCard(next.bg, next.node, true);
      next.resizeHandle.visible = true;
    }
    this.refreshPortVisibility();
    this.options.onSelectNode?.(id);
  }

  selectEdge(id: string | null) {
    if (this.selectedId) {
      const prev = this.nodeCards.get(this.selectedId);
      if (prev) {
        this.paintCard(prev.bg, prev.node, false);
        prev.resizeHandle.visible = false;
      }
      this.selectedId = null;
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
      this.drawEdge(g, g.parent as Container, edge, from, to, this.selectedEdgeId === edge.id);
    }
  }

  private refreshSelectionChrome() {
    this.refreshPortVisibility();
    const card = this.selectedId ? this.nodeCards.get(this.selectedId) : undefined;
    if (card) card.resizeHandle.visible = true;
  }

  moveSelectedBy(dx: number, dy: number): boolean {
    if (!this.selectedId) return false;
    const card = this.nodeCards.get(this.selectedId);
    if (!card) return false;
    const x = card.node.x + dx;
    const y = card.node.y + dy;
    card.node = { ...card.node, x, y };
    card.container.position.set(x, y);
    this.options.onMoveNodes?.([{ id: this.selectedId, x, y }]);
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
    if (!card) return;
    this.selectNode(id);
    // Prefer the live container pose — card.node can lag if a prior drag
    // committed visually before React/disk state caught up.
    this.dragging = {
      id,
      startX: event.global.x,
      startY: event.global.y,
      nodeX: card.container.x,
      nodeY: card.container.y,
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
    const boxW = Math.max(1, maxX - minX);
    const boxH = Math.max(1, maxY - minY);
    const screenW = this.app.screen.width || this.host.clientWidth || 800;
    const screenH = this.app.screen.height || this.host.clientHeight || 600;

    const scale = clamp(Math.min(screenW / boxW, screenH / boxH) * 0.88, MIN_SCALE, MAX_SCALE);
    this.world.scale.set(scale);
    this.world.position.set(
      screenW / 2 - (minX + boxW / 2) * scale,
      screenH / 2 - (minY + boxH / 2) * scale,
    );
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
      const card = this.nodeCards.get(drag.id);
      if (!card) return;
      const dx = (e.global.x - drag.startX) / this.world.scale.x;
      const dy = (e.global.y - drag.startY) / this.world.scale.y;
      if (Math.abs(dx) > 1 || Math.abs(dy) > 1) drag.moved = true;
      card.container.position.set(drag.nodeX + dx, drag.nodeY + dy);
      return;
    }
    if (!this.panning) return;
    const dx = e.global.x - this.panStart.x;
    const dy = e.global.y - this.panStart.y;
    this.world.position.set(this.panOrigin.x + dx, this.panOrigin.y + dy);
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
      const card = this.nodeCards.get(drag.id);
      this.dragging = null;
      if (card && drag.moved) {
        const x = card.container.x;
        const y = card.container.y;
        card.node = { ...card.node, x, y };
        this.suppressTapFor = drag.id;
        this.options.onMoveNodes?.([{ id: drag.id, x, y }]);
      }
    }
    this.panning = false;
  };

  private onWheel = (e: WheelEvent) => {
    e.preventDefault();
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
  };

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.cancelLink();
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

function withAlpha(hex: string, alpha: number): string {
  const m = /^#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})/.exec(hex);
  if (!m) return hex;
  const [r, g, b] = m.slice(1).map((h) => parseInt(h, 16));
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
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
