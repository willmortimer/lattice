import assert from "node:assert/strict";
import test from "node:test";

import {
  CENTER,
  NEAR,
  OUTLINE,
  VERTICES,
  project,
  translatePoint,
} from "../src/lib/lattice-geometry.mjs";

const roundedKey = ([x, y]) => `${x.toFixed(8)}:${y.toFixed(8)}`;

test("the near corner projects to the center of the unit-cell mark", () => {
  const [x, y] = project(CENTER);
  assert.ok(Math.abs(x) < 1e-10);
  assert.ok(Math.abs(y) < 1e-10);
});

test("the canonical mark has seven distinct visible nodes", () => {
  const origin = [1, 1, 1];
  const visibleNodes = [...VERTICES, CENTER]
    .map((point) => translatePoint(point, origin))
    .map(project)
    .map(roundedKey);

  assert.equal(new Set(visibleNodes).size, 7);
});

test("the visible mark graph is the six-edge silhouette plus its three near edges", () => {
  assert.equal(OUTLINE.length, 6);
  assert.equal(NEAR.length, 3);
  assert.deepEqual(NEAR.map(([from]) => from), [CENTER, CENTER, CENTER]);
});
