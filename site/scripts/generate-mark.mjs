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
 * Writes site/src/assets/lattice-mark.svg and prints the inline variants
 * (header/footer SVG with CSS variables, favicon data URI, desktop BrandMark
 * geometry) to stdout for pasting into components.
 */

import { writeFileSync } from "node:fs";
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

// 2. Inline variant for Layout.astro (CSS variables instead of hex)
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

// 3. Favicon data URI (dark rounded tile + simplified cell — no interior
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

// 4. Desktop BrandMark geometry (56 viewBox)
console.log("--- BrandMark (App.tsx, 56 viewBox) ---\n");
const b = layout(56, 22);
const [bcx, bcy] = b.pt(CENTER);
console.log(`far:     ${b.path(FAR)}`);
console.log(`grid:    ${b.path(GRID)}`);
console.log(`outline: ${b.path(OUTLINE)}`);
console.log(`near:    ${b.path(NEAR)}`);
console.log(`center:  cx=${bcx} cy=${bcy}`);
console.log(`vertices:${VERTICES.map((v) => b.pt(v).join(",")).join(" ")}`);
