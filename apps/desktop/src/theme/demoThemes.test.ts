import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { FONT_PACKS, THEME_DEFAULT_FONT_PACKS } from "./demoThemes";

/**
 * The browser demo mirrors the Rust theme catalog by hand. These tests fail
 * when a theme's `font_pack:` or a shipped pack id drifts from the YAML.
 */
const THEMES_DIR = join(__dirname, "../../../../themes");
const PACKS_DIR = join(THEMES_DIR, "font-packs");

function scalar(text: string, key: string): string | undefined {
  return new RegExp(`^${key}:\\s*(\\S+)\\s*$`, "m").exec(text)?.[1];
}

function themeFiles(): { id: string; fontPack: string }[] {
  return readdirSync(THEMES_DIR)
    .filter((name) => name.endsWith(".theme.yaml"))
    .map((name) => {
      const text = readFileSync(join(THEMES_DIR, name), "utf8");
      const id = scalar(text, "id");
      const fontPack = scalar(text, "font_pack");
      expect(id, `${name} has an id`).toBeDefined();
      expect(fontPack, `${name} has a font_pack`).toBeDefined();
      return { id: id!, fontPack: fontPack! };
    });
}

describe("demo theme catalog", () => {
  it("mirrors every theme's default font pack", () => {
    for (const { id, fontPack } of themeFiles()) {
      expect(THEME_DEFAULT_FONT_PACKS[id] ?? "lattice", `theme ${id}`).toBe(fontPack);
    }
  });

  it("ships every font pack the themes reference", () => {
    const packIds = readdirSync(PACKS_DIR)
      .filter((name) => name.endsWith(".font-pack.yaml"))
      .map((name) => name.replace(".font-pack.yaml", ""));

    expect(Object.keys(FONT_PACKS).sort()).toEqual([...packIds].sort());
    for (const { id, fontPack } of themeFiles()) {
      expect(packIds, `theme ${id} references a known pack`).toContain(fontPack);
    }
  });
});
