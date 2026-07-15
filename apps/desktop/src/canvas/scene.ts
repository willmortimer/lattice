import { Application, Container, FederatedPointerEvent, Graphics, Rectangle, Text } from "pixi.js";
import type { CanvasData, CanvasEdge, CanvasNode } from "./types";
import { classifyPath } from "./classify";
import { KIND_LABELS } from "../KindMark";
import * as colors from "./colors";

const MIN_SCALE = 0.1;
const MAX_SCALE = 3;
const DOUBLE_CLICK_MS = 400;
const CARD_RADIUS = 8;
const CARD_PADDING = 12;

interface CanvasSceneOptions {
  onOpenFile: (path: string) => void;
  onSelectNode?: (id: string | null) => void;
}

type Side = "top" | "right" | "bottom" | "left";

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

/** Parse a JSON Canvas node color: only hex is honored (see colors.ts note). */
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

  private nodeCards = new Map<string, { container: Container; bg: Graphics; node: CanvasNode }>();
  private selectedId: string | null = null;
  private lastTapAt = new Map<string, number>();

  private resizeObserver: ResizeObserver | null = null;
  private host: HTMLElement;
  private options: CanvasSceneOptions;

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
        await document.fonts.ready.catch(() => {});
        if (this.destroyed) {
          // destroy() ran while init was in flight; finish the teardown here,
          // now that the renderer actually exists.
          this.app.destroy(true, { children: true, texture: true });
          return;
        }
        this.initialized = true;
        this.setup();
      });
  }

  private setup() {
    this.host.appendChild(this.app.canvas);
    this.app.canvas.style.display = "block";
    this.app.canvas.style.touchAction = "none";

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
      }
    });
    this.resizeObserver.observe(this.host);
  }

  setData(data: CanvasData) {
    if (!this.initialized || this.destroyed) return;
    this.groupsLayer.removeChildren();
    this.edgesLayer.removeChildren();
    this.nodesLayer.removeChildren();
    this.nodeCards.clear();
    this.selectedId = null;

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

    const edgeGraphics = new Graphics();
    this.edgesLayer.addChild(edgeGraphics);
    for (const edge of data.edges) {
      const from = byId.get(edge.fromNode);
      const to = byId.get(edge.toNode);
      if (!from || !to) continue;
      this.drawEdge(edgeGraphics, edge, from, to);
    }

    this.fitToContent(data.nodes);
  }

  private buildGroup(node: CanvasNode & { type: "group" }): Container {
    const container = new Container();
    container.position.set(node.x, node.y);

    const accent = nodeAccent(node.color);
    const bg = new Graphics()
      .roundRect(0, 0, node.width, node.height, CARD_RADIUS + 4)
      .fill(accent ? withAlpha(accent, 0.08) : colors.BG_RAISE)
      .stroke({ width: 1, color: accent ?? colors.LINE_STRONG });
    container.addChild(bg);

    if (node.label) {
      const label = new Text({
        text: node.label,
        style: {
          fontFamily: colors.FONT_DISPLAY,
          fontSize: 14,
          fontWeight: "600",
          fill: colors.TEXT_SOFT,
        },
      });
      label.position.set(2, -22);
      container.addChild(label);
    }

    container.eventMode = "static";
    container.cursor = "default";
    container.hitArea = new Rectangle(0, 0, node.width, node.height);
    container.on("pointerdown", (e: FederatedPointerEvent) => e.stopPropagation());
    container.on("pointertap", () => this.selectNode(node.id));

    return container;
  }

  private buildCard(node: CanvasNode): { container: Container; bg: Graphics; node: CanvasNode } {
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
          fontFamily: colors.FONT_UI,
          fontSize: 13,
          fontWeight: "600",
          fill: colors.TEXT,
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
          fontFamily: colors.FONT_MONO,
          fontSize: 10,
          letterSpacing: 0.6,
          fill: colors.AMBER_DEEP,
        },
      });
      kindLabel.position.set(textX, CARD_PADDING + title.height + 6);
      container.addChild(kindLabel);
    } else if (node.type === "link") {
      const title = new Text({
        text: "Link",
        style: {
          fontFamily: colors.FONT_MONO,
          fontSize: 10,
          letterSpacing: 0.6,
          fill: colors.FAINT,
        },
      });
      title.position.set(textX, CARD_PADDING);
      container.addChild(title);

      const url = new Text({
        text: node.url,
        style: {
          fontFamily: colors.FONT_MONO,
          fontSize: 12,
          fill: colors.AMBER,
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
          fontFamily: colors.FONT_UI,
          fontSize: 12.5,
          lineHeight: 18,
          fill: colors.TEXT_SOFT,
          wordWrap: true,
          wordWrapWidth: textWidth,
          breakWords: true,
        },
      });
      bodyText.position.set(textX, CARD_PADDING);
      container.addChild(bodyText);
    }

    container.eventMode = "static";
    container.cursor = "pointer";
    container.hitArea = new Rectangle(0, 0, node.width, node.height);
    container.on("pointerdown", (e: FederatedPointerEvent) => e.stopPropagation());
    container.on("pointertap", () => this.onNodeTap(node));

    return { container, bg, node };
  }

  private paintCard(bg: Graphics, node: CanvasNode, selected: boolean) {
    bg
      .clear()
      .roundRect(0, 0, node.width, node.height, CARD_RADIUS)
      .fill(colors.PANEL)
      .stroke({ width: selected ? 2 : 1, color: selected ? colors.AMBER : colors.BORDER });
  }

  private drawEdge(g: Graphics, edge: CanvasEdge, from: CanvasNode, to: CanvasNode) {
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

    const stroke = nodeAccent(edge.color) ?? colors.LINE_STRONG;
    g.moveTo(start.x, start.y).bezierCurveTo(cp1.x, cp1.y, cp2.x, cp2.y, end.x, end.y).stroke({
      width: 1.5,
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

    if (edge.label) {
      const mid = bezierPoint(start, cp1, cp2, end, 0.5);
      const label = new Text({
        text: edge.label,
        style: {
          fontFamily: colors.FONT_UI,
          fontSize: 11,
          fill: colors.MUTED,
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
        .fill(colors.PANEL);
      this.edgesLayer.addChild(backdrop, label);
    }
  }

  private onNodeTap(node: CanvasNode) {
    this.selectNode(node.id);
    if (node.type !== "file") return;

    const now = performance.now();
    const last = this.lastTapAt.get(node.id) ?? 0;
    this.lastTapAt.set(node.id, now);
    if (now - last < DOUBLE_CLICK_MS) {
      this.lastTapAt.delete(node.id);
      this.options.onOpenFile(node.file);
    }
  }

  private selectNode(id: string | null) {
    if (this.selectedId === id) return;
    const prev = this.selectedId ? this.nodeCards.get(this.selectedId) : undefined;
    if (prev) this.paintCard(prev.bg, prev.node, false);

    this.selectedId = id;
    const next = id ? this.nodeCards.get(id) : undefined;
    if (next) this.paintCard(next.bg, next.node, true);

    this.options.onSelectNode?.(id);
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
    if (e.target !== this.app.stage) return;
    this.selectNode(null);
    this.panning = true;
    this.panStart = { x: e.global.x, y: e.global.y };
    this.panOrigin = { x: this.world.position.x, y: this.world.position.y };
  };

  private onStagePointerMove = (e: FederatedPointerEvent) => {
    if (!this.panning) return;
    const dx = e.global.x - this.panStart.x;
    const dy = e.global.y - this.panStart.y;
    this.world.position.set(this.panOrigin.x + dx, this.panOrigin.y + dy);
  };

  private onStagePointerUp = () => {
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
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
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
