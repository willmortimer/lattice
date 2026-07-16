import { useCallback, useEffect, useState } from "react";

const SETTINGS_KEY = "lattice.desktop.settings.v1";
const SETTINGS_EVENT = "lattice-settings-changed";

export interface AppSettings {
  editor: {
    autosaveDelayMs: number;
    spellcheck: boolean;
    slashCommands: boolean;
    showFrontmatter: boolean;
    linkClickBehavior: "navigate" | "inspect";
  };
  files: {
    restoreSession: boolean;
    reopenLastWorkspace: boolean;
    quickNoteDirectory: string;
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
  capabilities: {
    search: boolean;
    quickCapture: boolean;
    canvas: boolean;
    dataApps: boolean;
    externalOpen: boolean;
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

export const DEFAULT_SETTINGS: AppSettings = {
  editor: {
    autosaveDelayMs: 800,
    spellcheck: true,
    slashCommands: true,
    showFrontmatter: true,
    linkClickBehavior: "navigate",
  },
  files: {
    restoreSession: true,
    reopenLastWorkspace: true,
    quickNoteDirectory: "Inbox",
    confirmCloseWithUnsavedChanges: true,
  },
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
  capabilities: {
    search: true,
    quickCapture: true,
    canvas: true,
    dataApps: true,
    externalOpen: true,
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

function mergeSettings(input: Partial<AppSettings> | null | undefined): AppSettings {
  return {
    editor: { ...DEFAULT_SETTINGS.editor, ...input?.editor },
    files: { ...DEFAULT_SETTINGS.files, ...input?.files },
    keybindings: { ...DEFAULT_SETTINGS.keybindings, ...input?.keybindings },
    data: { ...DEFAULT_SETTINGS.data, ...input?.data },
    capabilities: { ...DEFAULT_SETTINGS.capabilities, ...input?.capabilities },
    performance: { ...DEFAULT_SETTINGS.performance, ...input?.performance },
    diagnostics: { ...DEFAULT_SETTINGS.diagnostics, ...input?.diagnostics },
  };
}

export function loadAppSettings(): AppSettings {
  try {
    return mergeSettings(JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "null"));
  } catch {
    return mergeSettings(null);
  }
}

export function saveAppSettings(settings: AppSettings): void {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  window.dispatchEvent(new CustomEvent(SETTINGS_EVENT, { detail: settings }));
}

export function useAppSettings() {
  const [settings, setSettingsState] = useState<AppSettings>(() => loadAppSettings());

  useEffect(() => {
    const onChange = (event: Event) => {
      setSettingsState(mergeSettings((event as CustomEvent<AppSettings>).detail));
    };
    window.addEventListener(SETTINGS_EVENT, onChange);
    return () => window.removeEventListener(SETTINGS_EVENT, onChange);
  }, []);

  const setSettings = useCallback(
    (next: AppSettings | ((current: AppSettings) => AppSettings)) => {
      setSettingsState((current) => {
        const resolved = typeof next === "function" ? next(current) : next;
        saveAppSettings(resolved);
        return resolved;
      });
    },
    [],
  );

  const resetSettings = useCallback(() => setSettings(mergeSettings(null)), [setSettings]);

  return { settings, setSettings, resetSettings };
}

export function matchesKeybinding(event: KeyboardEvent, binding: string): boolean {
  const parts = binding
    .split("+")
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean);
  const key = parts.at(-1);
  if (!key) return false;
  const wantsMod = parts.includes("mod");
  const wantsShift = parts.includes("shift");
  const wantsAlt = parts.includes("alt") || parts.includes("option");
  const wantsCtrl = parts.includes("ctrl") || parts.includes("control");
  const wantsMeta = parts.includes("meta") || parts.includes("cmd") || parts.includes("command");
  const eventKey = event.key.toLowerCase();
  const normalizedKey = key === "comma" ? "," : key;

  return (
    eventKey === normalizedKey &&
    (wantsMod ? event.metaKey || event.ctrlKey : true) &&
    (wantsShift ? event.shiftKey : !event.shiftKey) &&
    (wantsAlt ? event.altKey : !event.altKey) &&
    (wantsCtrl ? event.ctrlKey : true) &&
    (wantsMeta ? event.metaKey : true)
  );
}
