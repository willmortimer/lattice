import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  buildThemeMirror,
  persistThemeMirror,
  readThemeMirror,
  selectThemeMirrorEntry,
  THEME_MIRROR_KEY,
  type ResolvedThemePayload,
  type ThemeMirror,
} from "./apply";

/** Minimal in-memory localStorage for node vitest. */
function installMemoryStorage(): void {
  const store = new Map<string, string>();
  const memory = {
    getItem(key: string) {
      return store.has(key) ? store.get(key)! : null;
    },
    setItem(key: string, value: string) {
      store.set(key, String(value));
    },
    removeItem(key: string) {
      store.delete(key);
    },
    clear() {
      store.clear();
    },
  };
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: memory,
  });
}

function payload(
  overrides: Partial<ResolvedThemePayload> &
    Pick<ResolvedThemePayload, "id" | "appearance" | "background">,
): ResolvedThemePayload {
  return {
    name: overrides.id,
    sourcePath: `builtin:${overrides.id}.theme.yaml`,
    vars: {
      "--lt-bg": overrides.background,
      "--lt-accent": overrides.appearance === "light" ? "#0b57d0" : "#f5a623",
    },
    settings: {
      mode: "fixed",
      theme: overrides.id,
      pair: { dark: "lattice-slate", light: "lattice-paper" },
    },
    workspaceOverride: {},
    diagnostics: [],
    ...overrides,
  };
}

describe("theme mirror", () => {
  beforeEach(() => {
    installMemoryStorage();
  });

  afterEach(() => {
    Reflect.deleteProperty(globalThis, "localStorage");
  });

  it("selects byAppearance when mode is auto", () => {
    const mirror: ThemeMirror = {
      id: "lattice-slate",
      background: "#0a0d13",
      appearance: "dark",
      vars: { "--lt-bg": "#0a0d13" },
      updatedAt: 1,
      mode: "auto",
      byAppearance: {
        dark: {
          id: "lattice-slate",
          background: "#0a0d13",
          appearance: "dark",
          vars: { "--lt-bg": "#0a0d13" },
        },
        light: {
          id: "lattice-paper",
          background: "#f4f1ea",
          appearance: "light",
          vars: { "--lt-bg": "#f4f1ea" },
        },
      },
    };

    expect(selectThemeMirrorEntry(mirror, "light").id).toBe("lattice-paper");
    expect(selectThemeMirrorEntry(mirror, "dark").id).toBe("lattice-slate");
  });

  it("uses the last applied theme when mode is fixed", () => {
    const mirror: ThemeMirror = {
      id: "lattice-ember",
      background: "#1a0f0c",
      appearance: "dark",
      vars: { "--lt-bg": "#1a0f0c" },
      updatedAt: 1,
      mode: "fixed",
      byAppearance: {
        light: {
          id: "lattice-paper",
          background: "#f4f1ea",
          appearance: "light",
          vars: { "--lt-bg": "#f4f1ea" },
        },
      },
    };

    expect(selectThemeMirrorEntry(mirror, "light").id).toBe("lattice-ember");
  });

  it("records separate dark and light variants while building", () => {
    const dark = buildThemeMirror(
      payload({
        id: "lattice-slate",
        appearance: "dark",
        background: "#0a0d13",
        settings: {
          mode: "auto",
          theme: "lattice-slate",
          pair: { dark: "lattice-slate", light: "lattice-paper" },
        },
      }),
      null,
      1,
    );
    const light = buildThemeMirror(
      payload({
        id: "lattice-paper",
        appearance: "light",
        background: "#f7f4ee",
        settings: {
          mode: "auto",
          theme: "lattice-paper",
          pair: { dark: "lattice-slate", light: "lattice-paper" },
        },
      }),
      dark,
      2,
    );

    expect(light.mode).toBe("auto");
    expect(light.byAppearance?.dark?.id).toBe("lattice-slate");
    expect(light.byAppearance?.light?.id).toBe("lattice-paper");
    expect(light.id).toBe("lattice-paper");
  });

  it("round-trips persistThemeMirror", () => {
    persistThemeMirror({
      id: "nord",
      background: "#2e3440",
      appearance: "dark",
      vars: { "--lt-bg": "#2e3440" },
      updatedAt: 42,
      mode: "fixed",
    });
    expect(readThemeMirror()?.id).toBe("nord");
    expect(localStorage.getItem(THEME_MIRROR_KEY)).toContain("nord");
  });
});
