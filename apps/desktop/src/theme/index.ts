import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { hasTauri } from "../lib/ipc";
import {
  applyResolvedTheme,
  detectSystemAppearance,
  type ResolvedThemePayload,
  type ThemeCatalogPayload,
} from "./apply";
import { demoCatalog, demoSetAppearanceMode, demoSetTheme } from "./demoThemes";

export type {
  ResolvedThemePayload,
  ThemeCatalogPayload,
  ThemeSummaryPayload,
} from "./apply";
export {
  applyResolvedTheme,
  detectSystemAppearance,
  selectThemeMirrorEntry,
} from "./apply";

function workspaceArg(root: string | null | undefined): string | null {
  return root && root.length > 0 ? root : null;
}

/** Load theme catalog (list + resolved active theme). */
export async function loadThemeCatalog(
  workspaceRoot?: string | null,
): Promise<ThemeCatalogPayload> {
  const system = detectSystemAppearance();
  if (!hasTauri) {
    return demoCatalog(system, workspaceRoot);
  }
  return invoke<ThemeCatalogPayload>("list_themes", {
    system,
    workspaceRoot: workspaceArg(workspaceRoot),
  });
}

export async function setFixedTheme(
  themeId: string,
  workspaceRoot?: string | null,
): Promise<ThemeCatalogPayload> {
  const system = detectSystemAppearance();
  if (!hasTauri) {
    return demoSetTheme(themeId, system);
  }
  return invoke<ThemeCatalogPayload>("set_theme", {
    themeId,
    system,
    workspaceRoot: workspaceArg(workspaceRoot),
  });
}

export async function setAppearanceMode(
  mode: "fixed" | "auto",
  workspaceRoot?: string | null,
): Promise<ThemeCatalogPayload> {
  const system = detectSystemAppearance();
  if (!hasTauri) {
    return demoSetAppearanceMode(mode, system);
  }
  return invoke<ThemeCatalogPayload>("set_appearance_mode", {
    args: {
      mode,
      system,
      workspaceRoot: workspaceArg(workspaceRoot),
    },
  });
}

export async function refreshResolvedTheme(
  workspaceRoot?: string | null,
): Promise<ResolvedThemePayload> {
  const catalog = await loadThemeCatalog(workspaceRoot);
  applyResolvedTheme(catalog.resolved);
  return catalog.resolved;
}

/** Subscribe to theme file / settings changes (Tauri only). */
export async function startThemeWatch(
  workspaceRoot: string | null | undefined,
  onChange: () => void,
): Promise<() => void> {
  if (!hasTauri) {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => onChange();
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }

  await invoke("start_theme_watching", {
    workspaceRoot: workspaceArg(workspaceRoot),
  });

  const unlisten: UnlistenFn = await listen("theme-changed", () => {
    onChange();
  });

  return () => {
    unlisten();
    void invoke("stop_theme_watching");
  };
}
