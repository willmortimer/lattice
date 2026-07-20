#!/usr/bin/env node
/**
 * Procedural generator for the Lattice brand mark.
 *
 * The mark is a unit cell of a 3D lattice under an axonometric projection:
 * a cube stood on its vertex so the silhouette reads as a diamond/hexagon.
 * Every line is computed from integer lattice coordinates — nothing is drawn
 * by hand — so the same constants reproduce the mark at any scale.
 *
 *   node site/scripts/generate-mark.mjs
 *
 * Writes the site mark and canonical desktop app-icon source, then prints
 * inline variants (header/footer SVG with CSS variables, favicon data URI,
 * desktop BrandMark geometry) to stdout for pasting into components.
 */

import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// --- Projection ------------------------------------------------------------
// Axonometric projection of lattice space onto the plane. TILT = 30° is the
// isometric case, where the near corner (1,1,1) of the unit cell projects
// exactly onto the silhouette's center — the brightest node of the mark is
// literally the point closest to the viewer.
const TILT = (30 * Math.PI) / 180;
const AX = Math.cos(TILT);
const AY = Math.sin(TILT);

const project = ([x, y, z]) => [(x - z) * AX, (x + z) * AY - y];

// --- Unit-cell geometry ------------------------------------------------------
// Silhouette: the six outer edges of the projected cube.
const OUTLINE = [
  [[0, 1, 0], [1, 1, 0]],
  [[1, 1, 0], [1, 0, 0]],
  [[1, 0, 0], [1, 0, 1]],
  [[1, 0, 1], [0, 0, 1]],
  [[0, 0, 1], [0, 1, 1]],
  [[0, 1, 1], [0, 1, 0]],
];

// The "Y": three visible edges meeting at the near corner (1,1,1).
const NEAR = [
  [[1, 1, 1], [1, 1, 0]],
  [[1, 1, 1], [0, 1, 1]],
  [[1, 1, 1], [1, 0, 1]],
];

// Hidden edges meeting at the far corner (0,0,0) — drawn faint, so the cell
// reads as a transparent 3D lattice rather than a solid.
const FAR = [
  [[0, 0, 0], [1, 0, 0]],
  [[0, 0, 0], [0, 1, 0]],
  [[0, 0, 0], [0, 0, 1]],
];

// Lattice subdivision of the three visible faces at t = 1/2.
const t = 0.5;
const GRID = [
  // top face (y = 1)
  [[t, 1, 0], [t, 1, 1]],
  [[0, 1, t], [1, 1, t]],
  // right face (x = 1)
  [[1, t, 0], [1, t, 1]],
  [[1, 0, t], [1, 1, t]],
  // left face (z = 1)
  [[t, 0, 1], [t, 1, 1]],
  [[0, t, 1], [1, t, 1]],
];

// Nodes: the six silhouette vertices, plus the near corner as the bright center.
const VERTICES = [
  [0, 1, 0],
  [1, 1, 0],
  [1, 0, 0],
  [1, 0, 1],
  [0, 0, 1],
  [0, 1, 1],
];
const CENTER = [1, 1, 1];

// --- SVG emission ------------------------------------------------------------
const r2 = (n) => Math.round(n * 100) / 100;

function layout(viewSize, scale) {
  const c = viewSize / 2;
  const pt = (p) => {
    const [px, py] = project(p);
    return [r2(c + px * scale), r2(c + py * scale)];
  };
  const seg = ([a, b]) => {
    const [x1, y1] = pt(a);
    const [x2, y2] = pt(b);
    return `M${x1} ${y1}L${x2} ${y2}`;
  };
  const path = (segs) => segs.map(seg).join("");
  return { pt, path };
}

function markSvg({ viewSize, scale, stroke, amber, bright, withAria }) {
  const { pt, path } = layout(viewSize, scale);
  const [cx, cy] = pt(CENTER);
  const dots = VERTICES.map(pt)
    .map(([x, y]) => `<circle cx="${x}" cy="${y}" r="${r2(scale * 0.125)}"/>`)
    .join("");
  const aria = withAria ? ` role="img" aria-label="Lattice"` : ` aria-hidden="true"`;
  return `<svg width="${viewSize}" height="${viewSize}" viewBox="0 0 ${viewSize} ${viewSize}" fill="none" xmlns="http://www.w3.org/2000/svg"${aria}>
  <g stroke="${amber}" stroke-linecap="round">
    <path d="${path(FAR)}" stroke-width="${r2(stroke * 0.7)}" opacity="0.28"/>
    <path d="${path(GRID)}" stroke-width="${r2(stroke * 0.75)}" opacity="0.45"/>
    <path d="${path(OUTLINE)}" stroke-width="${stroke}" opacity="0.9"/>
    <path d="${path(NEAR)}" stroke-width="${stroke}" opacity="0.95"/>
  </g>
  <g fill="${bright}">${dots}</g>
  <circle cx="${cx}" cy="${cy}" r="${r2(scale * 0.19)}" fill="${amber}"/>
</svg>
`;
}

function appIconSvg() {
  const viewSize = 1024;
  const scale = 330;
  const stroke = 34;
  const { pt, path } = layout(viewSize, scale);
  const [cx, cy] = pt(CENTER);
  const dots = VERTICES.map(pt)
    .map(([x, y]) => `<circle cx="${x}" cy="${y}" r="38"/>`)
    .join("");

  return `<svg width="${viewSize}" height="${viewSize}" viewBox="0 0 ${viewSize} ${viewSize}" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Lattice">
  <defs>
    <linearGradient id="tile" x1="180" y1="90" x2="850" y2="930" gradientUnits="userSpaceOnUse">
      <stop stop-color="#13253A"/>
      <stop offset="0.55" stop-color="#0B1828"/>
      <stop offset="1" stop-color="#07111D"/>
    </linearGradient>
    <radialGradient id="signal" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(${cx} ${cy}) rotate(90) scale(280)">
      <stop stop-color="#F5A623" stop-opacity="0.16"/>
      <stop offset="1" stop-color="#F5A623" stop-opacity="0"/>
    </radialGradient>
    <clipPath id="tile-clip">
      <rect x="48" y="48" width="928" height="928" rx="214"/>
    </clipPath>
  </defs>
  <rect x="48" y="48" width="928" height="928" rx="214" fill="url(#tile)"/>
  <g clip-path="url(#tile-clip)">
    <path d="M96 290H928M96 512H928M96 734H928M290 96V928M512 96V928M734 96V928" stroke="#88A4C2" stroke-width="5" opacity="0.1"/>
    <circle cx="${cx}" cy="${cy}" r="280" fill="url(#signal)"/>
  </g>
  <g stroke="#F5A623" stroke-linecap="round" stroke-linejoin="round">
    <path d="${path(FAR)}" stroke-width="${r2(stroke * 0.7)}" opacity="0.24"/>
    <path d="${path(GRID)}" stroke-width="${r2(stroke * 0.72)}" opacity="0.4"/>
    <path d="${path(OUTLINE)}" stroke-width="${stroke}" opacity="0.94"/>
    <path d="${path(NEAR)}" stroke-width="${stroke}" opacity="0.98"/>
  </g>
  <g fill="#FFCE8A">${dots}</g>
  <circle cx="${cx}" cy="${cy}" r="62" fill="#F5A623"/>
  <rect x="50.5" y="50.5" width="923" height="923" rx="211.5" stroke="#8FB3D8" stroke-width="5" opacity="0.22"/>
</svg>
`;
}

// macOS menu-bar / tray template: black geometry, alpha carries hierarchy, no
// tile background. NSImage template mode tints this to match the menu bar.
function trayTemplateSvg() {
  const viewSize = 64;
  const scale = 22;
  const stroke = 2.4;
  const { pt, path } = layout(viewSize, scale);
  const [cx, cy] = pt(CENTER);
  const dots = VERTICES.map(pt)
    .map(([x, y]) => `<circle cx="${x}" cy="${y}" r="${r2(scale * 0.11)}"/>`)
    .join("");
  return `<svg width="${viewSize}" height="${viewSize}" viewBox="0 0 ${viewSize} ${viewSize}" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Lattice">
  <!-- Template mark: black + alpha only. Do not add a rounded tile. -->
  <g stroke="#000" stroke-linecap="round" stroke-linejoin="round">
    <path d="${path(FAR)}" stroke-width="${r2(stroke * 0.7)}" opacity="0.35"/>
    <path d="${path(GRID)}" stroke-width="${r2(stroke * 0.75)}" opacity="0.5"/>
    <path d="${path(OUTLINE)}" stroke-width="${stroke}" opacity="0.95"/>
    <path d="${path(NEAR)}" stroke-width="${stroke}" opacity="1"/>
  </g>
  <g fill="#000">${dots}</g>
  <circle cx="${cx}" cy="${cy}" r="${r2(scale * 0.16)}" fill="#000"/>
</svg>
`;
}

// --- PNG (tray template raster) ----------------------------------------------
function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = c & 1 ? (0xedb88320 ^ (c >>> 1)) : c >>> 1;
  }
  return ~c >>> 0;
}

function pngChunk(type, data) {
  const typeBuf = Buffer.from(type, "ascii");
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])));
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

function encodePngRgba(width, height, rgba) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", deflateSync(raw, { level: 9 })),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

function distToSegment(px, py, x1, y1, x2, y2) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len2 = dx * dx + dy * dy;
  if (len2 === 0) return Math.hypot(px - x1, py - y1);
  let t = ((px - x1) * dx + (py - y1) * dy) / len2;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(px - (x1 + t * dx), py - (y1 + t * dy));
}

function strokeCoverage(dist, halfWidth) {
  const edge = 0.65;
  if (dist <= halfWidth - edge) return 1;
  if (dist >= halfWidth + edge) return 0;
  return 1 - (dist - (halfWidth - edge)) / (2 * edge);
}

function rasterizeTrayTemplate(size) {
  const scale = size * (22 / 64);
  const stroke = size * (2.4 / 64);
  const { pt } = layout(size, scale);
  const segs = (pairs) => pairs.map(([a, b]) => [pt(a), pt(b)]);
  const layers = [
    { segments: segs(FAR), half: stroke * 0.35, alpha: 0.35 },
    { segments: segs(GRID), half: stroke * 0.375, alpha: 0.5 },
    { segments: segs(OUTLINE), half: stroke * 0.5, alpha: 0.95 },
    { segments: segs(NEAR), half: stroke * 0.5, alpha: 1 },
  ];
  const dots = [...VERTICES.map(pt), pt(CENTER)];
  const nodeR = [
    ...VERTICES.map(() => scale * 0.11),
    scale * 0.16,
  ];

  const rgba = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const px = x + 0.5;
      const py = y + 0.5;
      let a = 0;
      for (const layer of layers) {
        let cover = 0;
        for (const [[x1, y1], [x2, y2]] of layer.segments) {
          cover = Math.max(
            cover,
            strokeCoverage(distToSegment(px, py, x1, y1, x2, y2), layer.half),
          );
        }
        a = Math.max(a, cover * layer.alpha);
      }
      for (let i = 0; i < dots.length; i++) {
        const [cx, cy] = dots[i];
        const cover = strokeCoverage(Math.hypot(px - cx, py - cy), nodeR[i]);
        a = Math.max(a, cover);
      }
      if (a <= 0) continue;
      const i = (y * size + x) * 4;
      // Template images: black RGB, shape in alpha. macOS tints at draw time.
      rgba[i] = 0;
      rgba[i + 1] = 0;
      rgba[i + 2] = 0;
      rgba[i + 3] = Math.round(Math.min(1, a) * 255);
    }
  }
  return encodePngRgba(size, size, rgba);
}

// --- Outputs -----------------------------------------------------------------
const here = dirname(fileURLToPath(import.meta.url));

// 1. Standalone asset (site/src/assets/lattice-mark.svg)
const asset = markSvg({
  viewSize: 32,
  scale: 13.2,
  stroke: 1.4,
  amber: "#f5a623",
  bright: "#ffce8a",
  withAria: true,
});
writeFileSync(join(here, "../src/assets/lattice-mark.svg"), asset);
console.log("wrote site/src/assets/lattice-mark.svg\n");

// 2. Canonical desktop source. Platform sizes are generated from this SVG by
// the Tauri CLI so the app icon cannot drift from the algorithmic kindmark.
const generatedDir = join(here, "../../design/generated");
mkdirSync(generatedDir, { recursive: true });
writeFileSync(join(generatedDir, "lattice-app-icon.svg"), appIconSvg());
console.log("wrote design/generated/lattice-app-icon.svg\n");

// 2b. Menu-bar / tray template (black + alpha). Copied into Tauri icons/ for
// NSStatusItem; the Dock/app icon above stays the full-color tile.
writeFileSync(join(generatedDir, "lattice-tray-icon.svg"), trayTemplateSvg());
const trayPng = rasterizeTrayTemplate(64);
const trayIconPath = join(here, "../../apps/desktop/src-tauri/icons/tray-template.png");
writeFileSync(join(generatedDir, "lattice-tray-icon.png"), trayPng);
writeFileSync(trayIconPath, trayPng);
console.log("wrote design/generated/lattice-tray-icon.svg");
console.log("wrote design/generated/lattice-tray-icon.png");
console.log("wrote apps/desktop/src-tauri/icons/tray-template.png\n");

// 3. Inline variant for Layout.astro (CSS variables instead of hex)
console.log("--- inline (Layout.astro) ---\n");
console.log(
  markSvg({
    viewSize: 32,
    scale: 13.2,
    stroke: 1.4,
    amber: "var(--l-amber)",
    bright: "var(--l-amber-bright)",
    withAria: false,
  }),
);

// 4. Favicon data URI (dark rounded tile + simplified cell — no interior
// grid, which muddies at 16px)
const { path: fpath, pt: fpt } = layout(32, 12.2);
const [fcx, fcy] = fpt(CENTER);
const favicon =
  `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>` +
  `<rect width='32' height='32' rx='7' fill='%230a0d13'/>` +
  `<g stroke='%23f5a623' stroke-linecap='round' fill='none'>` +
  `<path d='${fpath(OUTLINE)}' stroke-width='1.6' opacity='0.9'/>` +
  `<path d='${fpath(NEAR)}' stroke-width='1.6'/>` +
  `</g>` +
  `<circle cx='${fcx}' cy='${fcy}' r='2.3' fill='%23f5a623'/>` +
  `</svg>`;
console.log("--- favicon href ---\n");
console.log(`data:image/svg+xml,${favicon}\n`);

// 5. Desktop BrandMark geometry (56 viewBox)
console.log("--- BrandMark (App.tsx, 56 viewBox) ---\n");
const b = layout(56, 22);
const [bcx, bcy] = b.pt(CENTER);
console.log(`far:     ${b.path(FAR)}`);
console.log(`grid:    ${b.path(GRID)}`);
console.log(`outline: ${b.path(OUTLINE)}`);
console.log(`near:    ${b.path(NEAR)}`);
console.log(`center:  cx=${bcx} cy=${bcy}`);
console.log(`vertices:${VERTICES.map((v) => b.pt(v).join(",")).join(" ")}`);
