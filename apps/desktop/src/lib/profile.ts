import { invoke } from "@tauri-apps/api/core";

import { hasTauri } from "./ipc";
import type { WorkspaceSnapshot } from "../types";
import {
  resourceTreeCollapseStorageKey,
  serializeResourceTreeCollapseState,
  type ResourceTreeCollapseState,
} from "./treeCollapse";

export interface RecentWorkspace {
  root: string;
  title: string;
  openedAt: number;
}

export interface DesktopSession {
  root: string;
  tabs: string[];
  active: string | null;
  activity: string | null;
  inspector: boolean;
}

export interface SettingsDiagnostic {
  path: string;
  code: string;
  message: string;
  severity: "warning" | "error";
}

export interface ProfileNotice {
  code: string;
  title: string;
  message: string;
  path: string | null;
}

export interface DesktopSettings {
  format: string;
  version: number;
  editor: {
    autosaveDelayMs: number;
    spellcheck: boolean;
    slashCommands: boolean;
    showFrontmatter: boolean;
    linkClickBehavior: "navigate" | "inspect";
  };
  files: {
    confirmCloseWithUnsavedChanges: boolean;
  };
  keybindings: {
    search: string;
    commandPalette: string;
    quickNote: string;
    newPage: string;
    settings: string;
  };
  data: {
    rowHeight: "compact" | "comfortable" | "spacious";
    pageSize: 100 | 250 | 500;
    showRowNumbers: boolean;
    zebraRows: boolean;
    defaultSortDirection: "asc" | "desc";
  };
  performance: {
    maxOpenTabs: number;
    suspendInactiveResources: boolean;
    reducedMotion: "system" | "always" | "never";
    rendererCache: "conservative" | "balanced" | "aggressive";
  };
  diagnostics: {
    nativeContextMenus: boolean;
    commandTimings: boolean;
    verboseErrors: boolean;
    showRendererStats: boolean;
  };
}

export interface WorkspaceStartupSettings {
  format: string;
  version: number;
  defaultWorkspace: string | null;
  reopenLastWorkspace: boolean;
  restoreSession: boolean;
  /** Brief branded splash before revealing the shell on launch. */
  showStartupSplash: boolean;
}

export interface ProfileSnapshot {
  settings: {
    desktop: DesktopSettings;
    workspaces: WorkspaceStartupSettings;
    desktopRevision: string | null;
    workspacesRevision: string | null;
    diagnostics: SettingsDiagnostic[];
  };
  recents: RecentWorkspace[];
  sidebarWidth: number | null;
  /** Collapsed resource-tree folder paths keyed by workspace id. */
  resourceTreeCollapsedByWorkspace: ResourceTreeCollapseState;
  effectiveDefaultWorkspace: string | null;
  hasValidConfiguredDefault: boolean;
  homeRoot: string;
  workspacesDirectory: string;
  notices: ProfileNotice[];
}

const DEMO_PROFILE_KEY = "lattice.browser.profile.v1";

export function defaultDesktopSettings(): DesktopSettings {
  return {
    format: "lattice-desktop-settings",
    version: 1,
    editor: {
      autosaveDelayMs: 800,
      spellcheck: true,
      slashCommands: true,
      showFrontmatter: true,
      linkClickBehavior: "navigate",
    },
    files: { confirmCloseWithUnsavedChanges: true },
    keybindings: {
      search: "Mod+K",
      commandPalette: "Mod+P",
      quickNote: "Mod+N",
      newPage: "Mod+Shift+N",
      settings: "Mod+,",
    },
    data: {
      rowHeight: "comfortable",
      pageSize: 500,
      showRowNumbers: true,
      zebraRows: false,
      defaultSortDirection: "asc",
    },
    performance: {
      maxOpenTabs: 12,
      suspendInactiveResources: true,
      reducedMotion: "system",
      rendererCache: "balanced",
    },
    diagnostics: {
      nativeContextMenus: true,
      commandTimings: false,
      verboseErrors: false,
      showRendererStats: false,
    },
  };
}

export function defaultWorkspaceStartupSettings(): WorkspaceStartupSettings {
  return {
    format: "lattice-workspace-settings",
    version: 1,
    defaultWorkspace: null,
    reopenLastWorkspace: true,
    restoreSession: true,
    showStartupSplash: true,
  };
}

function demoProfile(): ProfileSnapshot {
  const defaults: ProfileSnapshot = {
    settings: {
      desktop: defaultDesktopSettings(),
      workspaces: defaultWorkspaceStartupSettings(),
      desktopRevision: null,
      workspacesRevision: null,
      diagnostics: [],
    },
    recents: [],
    sidebarWidth: null,
    resourceTreeCollapsedByWorkspace: {},
    effectiveDefaultWorkspace: null,
    hasValidConfiguredDefault: false,
    homeRoot: "~/Lattice",
    workspacesDirectory: "~/Lattice/Workspaces",
    notices: [],
  };
  try {
    const saved = JSON.parse(localStorage.getItem(DEMO_PROFILE_KEY) ?? "null");
    return saved ? normalizeProfile({ ...defaults, ...saved }) : defaults;
  } catch {
    return defaults;
  }
}

function saveDemoProfile(profile: ProfileSnapshot) {
  localStorage.setItem(DEMO_PROFILE_KEY, JSON.stringify(profile));
}

function parseLegacy(value: string | null, fallback: unknown) {
  if (!value) return fallback;
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
}

export async function loadProfile(): Promise<ProfileSnapshot> {
  if (!hasTauri) return demoProfile();

  const legacySettings = localStorage.getItem("lattice.desktop.settings.v1");
  const legacyRecents = localStorage.getItem("lattice.recentWorkspaces");
  const legacySidebar = Number(localStorage.getItem("lattice.sidebarWidth"));
  const payload = {
    desktopSettings: parseLegacy(legacySettings, null),
    recents: parseLegacy(legacyRecents, []),
    sessions: Object.keys(localStorage)
      .filter((key) => key.startsWith("lattice.desktop.session:"))
      .flatMap((key) => {
        try {
          const root = key.slice("lattice.desktop.session:".length);
          return [{ root, ...JSON.parse(localStorage.getItem(key) ?? "{}") }];
        } catch {
          return [];
        }
      }),
    sidebarWidth: Number.isFinite(legacySidebar) ? legacySidebar : null,
  };
  const imported = await invoke<ProfileSnapshot>("import_legacy_profile", { payload });
  localStorage.removeItem("lattice.desktop.settings.v1");
  localStorage.removeItem("lattice.recentWorkspaces");
  localStorage.removeItem("lattice.sidebarWidth");
  Object.keys(localStorage)
    .filter((key) => key.startsWith("lattice.desktop.session:"))
    .forEach((key) => localStorage.removeItem(key));
  return normalizeProfile(imported);
}

function normalizeProfile(profile: ProfileSnapshot): ProfileSnapshot {
  return {
    ...profile,
    resourceTreeCollapsedByWorkspace: profile.resourceTreeCollapsedByWorkspace ?? {},
    settings: {
      ...profile.settings,
      workspaces: {
        ...defaultWorkspaceStartupSettings(),
        ...profile.settings.workspaces,
      },
    },
  };
}

export async function saveDesktopSettings(
  profile: ProfileSnapshot,
  settings: DesktopSettings,
): Promise<ProfileSnapshot> {
  if (!hasTauri) {
    const next = {
      ...profile,
      settings: { ...profile.settings, desktop: settings },
    };
    saveDemoProfile(next);
    return next;
  }
  return invoke("save_desktop_settings", {
    settings,
    expectedRevision: profile.settings.desktopRevision,
  });
}

export async function saveWorkspaceStartupSettings(
  profile: ProfileSnapshot,
  settings: WorkspaceStartupSettings,
): Promise<ProfileSnapshot> {
  if (!hasTauri) {
    const next = {
      ...profile,
      settings: { ...profile.settings, workspaces: settings },
    };
    saveDemoProfile(next);
    return next;
  }
  return invoke("save_workspace_startup_settings", {
    settings,
    expectedRevision: profile.settings.workspacesRevision,
  });
}

export async function rememberWorkspace(
  profile: ProfileSnapshot,
  workspace: WorkspaceSnapshot,
): Promise<ProfileSnapshot> {
  if (!hasTauri) {
    const recent = { root: workspace.root, title: workspace.title, openedAt: Date.now() };
    const next = {
      ...profile,
      recents: [recent, ...profile.recents.filter((item) => item.root !== recent.root)].slice(0, 8),
    };
    saveDemoProfile(next);
    return next;
  }
  const recents = await invoke<RecentWorkspace[]>("remember_workspace", {
    root: workspace.root,
    title: workspace.title,
  });
  return { ...profile, recents };
}

export async function loadSession(root: string): Promise<DesktopSession | null> {
  if (!hasTauri) return null;
  return invoke("load_desktop_session", { root });
}

export async function saveSession(session: DesktopSession): Promise<void> {
  if (!hasTauri) return;
  await invoke("save_desktop_session", { session });
}

export async function saveSidebarWidth(width: number): Promise<void> {
  if (!hasTauri) return;
  await invoke("set_profile_ui_value", { key: "sidebar-width", value: String(width) });
}

export async function saveResourceTreeCollapsed(
  profile: ProfileSnapshot,
  state: ResourceTreeCollapseState,
): Promise<ProfileSnapshot> {
  const resourceTreeCollapsedByWorkspace = { ...state };
  if (!hasTauri) {
    const next = { ...profile, resourceTreeCollapsedByWorkspace };
    saveDemoProfile(next);
    return next;
  }
  await invoke("set_profile_ui_value", {
    key: resourceTreeCollapseStorageKey(),
    value: serializeResourceTreeCollapseState(resourceTreeCollapsedByWorkspace),
  });
  return { ...profile, resourceTreeCollapsedByWorkspace };
}

export async function clearRecents(profile: ProfileSnapshot): Promise<ProfileSnapshot> {
  if (!hasTauri) {
    const next = { ...profile, recents: [] };
    saveDemoProfile(next);
    return next;
  }
  const recents = await invoke<RecentWorkspace[]>("clear_recent_workspaces");
  return { ...profile, recents };
}

export async function removeRecent(
  profile: ProfileSnapshot,
  root: string,
): Promise<ProfileSnapshot> {
  if (!hasTauri) {
    const next = { ...profile, recents: profile.recents.filter((item) => item.root !== root) };
    saveDemoProfile(next);
    return next;
  }
  const recents = await invoke<RecentWorkspace[]>("remove_recent_workspace", { root });
  return { ...profile, recents };
}
