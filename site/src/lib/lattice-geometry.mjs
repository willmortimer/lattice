/**
 * Canonical axonometric geometry for the Lattice mark and its larger scenes.
 *
 * Keep the geometry in lattice coordinates for as long as possible. Consumers
 * may translate or scale the unit cell, but the graph itself stays shared.
 */

export const TILT = (30 * Math.PI) / 180;

/** @typedef {[number, number, number]} LatticePoint */
/** @typedef {[LatticePoint, LatticePoint]} LatticeSegment */

/**
 * @param {LatticePoint} point
 * @param {number} [tilt]
 * @returns {[number, number]}
 */
export function project([x, y, z], tilt = TILT) {
  const ax = Math.cos(tilt);
  const ay = Math.sin(tilt);
  return [(x - z) * ax, (x + z) * ay - y];
}

// Six silhouette edges of the projected unit cell.
/** @type {LatticeSegment[]} */
export const OUTLINE = [
  [[0, 1, 0], [1, 1, 0]],
  [[1, 1, 0], [1, 0, 0]],
  [[1, 0, 0], [1, 0, 1]],
  [[1, 0, 1], [0, 0, 1]],
  [[0, 0, 1], [0, 1, 1]],
  [[0, 1, 1], [0, 1, 0]],
];

// Three visible edges meeting at the near corner.
/** @type {LatticeSegment[]} */
export const NEAR = [
  [[1, 1, 1], [1, 1, 0]],
  [[1, 1, 1], [0, 1, 1]],
  [[1, 1, 1], [1, 0, 1]],
];

// Three hidden edges meeting at the far corner.
/** @type {LatticeSegment[]} */
export const FAR = [
  [[0, 0, 0], [1, 0, 0]],
  [[0, 0, 0], [0, 1, 0]],
  [[0, 0, 0], [0, 0, 1]],
];

// Subdivision of the three visible faces at t = 1/2.
const t = 0.5;
/** @type {LatticeSegment[]} */
export const GRID = [
  [[t, 1, 0], [t, 1, 1]],
  [[0, 1, t], [1, 1, t]],
  [[1, t, 0], [1, t, 1]],
  [[1, 0, t], [1, 1, t]],
  [[t, 0, 1], [t, 1, 1]],
  [[0, t, 1], [1, t, 1]],
];

// Six silhouette vertices plus the near corner at the projected center.
/** @type {LatticePoint[]} */
export const VERTICES = [
  [0, 1, 0],
  [1, 1, 0],
  [1, 0, 0],
  [1, 0, 1],
  [0, 0, 1],
  [0, 1, 1],
];
/** @type {LatticePoint} */
export const CENTER = [1, 1, 1];

/**
 * @param {LatticePoint} point
 * @param {LatticePoint} offset
 * @returns {LatticePoint}
 */
export function translatePoint(point, offset) {
  return [point[0] + offset[0], point[1] + offset[1], point[2] + offset[2]];
}

/**
 * @param {LatticeSegment[]} segments
 * @param {LatticePoint} offset
 * @returns {LatticeSegment[]}
 */
export function translateSegments(segments, offset) {
  return segments.map(([from, to]) => [
    translatePoint(from, offset),
    translatePoint(to, offset),
  ]);
}
