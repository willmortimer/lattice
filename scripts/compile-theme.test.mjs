#!/usr/bin/env node
/**
 * Smoke tests for scripts/compile-theme.mjs (YAML subset + validation).
 * Run: node --test scripts/compile-theme.test.mjs
 */

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import assert from "node:assert/strict";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const COMPILE = join(ROOT, "scripts", "compile-theme.mjs");

test("default Lattice Slate theme compiles", () => {
  const r = spawnSync(process.execPath, [COMPILE], {
    cwd: ROOT,
    encoding: "utf8",
  });
  assert.equal(r.status, 0, r.stderr || r.stdout);
  const css = readFileSync(join(ROOT, "apps/desktop/src/theme-tokens.css"), "utf8");
  assert.match(css, /--lt-bg: #0a0d13;/);
  assert.match(css, /--lt-accent: #f5a623;/);
  assert.match(css, /color-mix\(in oklch, var\(--lt-accent\) 10%, transparent\)/);
  assert.match(css, /--l-amber: var\(--lt-accent\);/);

  const ts = readFileSync(join(ROOT, "apps/desktop/src/theme-tokens.ts"), "utf8");
  assert.match(ts, /accent: "#f5a623"/);
  assert.match(ts, /export const AMBER = LT\.accent;/);
});

test("rejects unknown palette refs", () => {
  const dir = mkdtempSync(join(tmpdir(), "lt-theme-"));
  try {
    const bad = join(dir, "bad.theme.yaml");
    writeFileSync(
      bad,
      `
name: Bad
id: bad
appearance: dark
palette:
  ground: "#000000"
roles:
  bg: $missing
  bg_raise: "#111"
  panel: "#222"
  slate: "#333"
  text: "#fff"
  text_soft: "#eee"
  muted: "#ccc"
  faint: "#999"
  accent: "#f00"
  accent_bright: "#f88"
  accent_deep: "#a00"
  danger: "#f66"
  shadow: "#000"
fonts:
  display: Serif
  ui: Sans
  mono: Mono
shape:
  radius: 9px
  radius_sm: 6px
  radius_lg: 14px
  grid: 34px
  titlebar: 38px
  max_width: 1140px
`,
    );
    const r = spawnSync(process.execPath, [COMPILE, bad], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.notEqual(r.status, 0);
    assert.match(r.stderr + r.stdout, /Unknown palette ref|missing/i);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
