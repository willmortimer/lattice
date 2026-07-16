import { useCallback, useEffect, useRef, useState } from "react";

import {
  defaultDesktopSettings,
  defaultWorkspaceStartupSettings,
  loadProfile,
  clearRecents as clearProfileRecents,
  rememberWorkspace as persistRecentWorkspace,
  removeRecent as removeProfileRecent,
  saveDesktopSettings,
  saveWorkspaceStartupSettings,
  type DesktopSettings,
  type ProfileSnapshot,
  type WorkspaceStartupSettings,
} from "../lib/profile";
import type { WorkspaceSnapshot } from "../types";

export type AppSettings = DesktopSettings;
export const DEFAULT_SETTINGS = defaultDesktopSettings();

function emptyProfile(): ProfileSnapshot {
  return {
    settings: {
      desktop: defaultDesktopSettings(),
      workspaces: defaultWorkspaceStartupSettings(),
      desktopRevision: null,
      workspacesRevision: null,
      diagnostics: [],
    },
    recents: [],
    sidebarWidth: null,
    effectiveDefaultWorkspace: null,
    hasValidConfiguredDefault: false,
    homeRoot: "~/Lattice",
    workspacesDirectory: "~/Lattice/Workspaces",
    notices: [],
  };
}

export function useAppSettings() {
  const [profile, setProfile] = useState<ProfileSnapshot>(emptyProfile);
  const [ready, setReady] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const profileRef = useRef(profile);
  const desktopSaveTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const startupSaveTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  profileRef.current = profile;

  useEffect(() => {
    let cancelled = false;
    void loadProfile()
      .then((next) => {
        if (!cancelled) setProfile(next);
      })
      .catch((error) => {
        if (!cancelled) setSaveError(String(error));
      })
      .finally(() => {
        if (!cancelled) setReady(true);
      });
    return () => {
      cancelled = true;
      if (desktopSaveTimer.current) window.clearTimeout(desktopSaveTimer.current);
      if (startupSaveTimer.current) window.clearTimeout(startupSaveTimer.current);
    };
  }, []);

  const setSettings = useCallback(
    (next: AppSettings | ((current: AppSettings) => AppSettings)) => {
      setProfile((current) => {
        const resolved =
          typeof next === "function" ? next(current.settings.desktop) : next;
        const optimistic = {
          ...current,
          settings: { ...current.settings, desktop: resolved },
        };
        if (desktopSaveTimer.current) window.clearTimeout(desktopSaveTimer.current);
        desktopSaveTimer.current = window.setTimeout(() => {
          const base = profileRef.current;
          const value = base.settings.desktop;
          void saveDesktopSettings(base, value)
            .then((saved) => {
              setProfile((latest) =>
                latest.settings.desktop === value
                  ? saved
                  : {
                      ...saved,
                      settings: { ...saved.settings, desktop: latest.settings.desktop },
                    },
              );
            })
            .catch((error) => setSaveError(String(error)));
        }, 180);
        return optimistic;
      });
    },
    [],
  );

  const setStartup = useCallback(
    (
      next:
        | WorkspaceStartupSettings
        | ((current: WorkspaceStartupSettings) => WorkspaceStartupSettings),
    ) => {
      setProfile((current) => {
        const resolved =
          typeof next === "function" ? next(current.settings.workspaces) : next;
        const optimistic = {
          ...current,
          settings: { ...current.settings, workspaces: resolved },
        };
        if (startupSaveTimer.current) window.clearTimeout(startupSaveTimer.current);
        startupSaveTimer.current = window.setTimeout(() => {
          const base = profileRef.current;
          const value = base.settings.workspaces;
          void saveWorkspaceStartupSettings(base, value)
            .then((saved) => {
              setProfile((latest) =>
                latest.settings.workspaces === value
                  ? saved
                  : {
                      ...saved,
                      settings: { ...saved.settings, workspaces: latest.settings.workspaces },
                    },
              );
            })
            .catch((error) => setSaveError(String(error)));
        }, 180);
        return optimistic;
      });
    },
    [],
  );

  const rememberWorkspace = useCallback((workspace: WorkspaceSnapshot) => {
    setProfile((current) => {
      void persistRecentWorkspace(current, workspace)
        .then(setProfile)
        .catch((error) => setSaveError(String(error)));
      return current;
    });
  }, []);

  const clearRecents = useCallback(() => {
    setProfile((current) => {
      void clearProfileRecents(current)
        .then(setProfile)
        .catch((error) => setSaveError(String(error)));
      return current;
    });
  }, []);

  const removeRecent = useCallback((root: string) => {
    setProfile((current) => {
      void removeProfileRecent(current, root)
        .then(setProfile)
        .catch((error) => setSaveError(String(error)));
      return current;
    });
  }, []);

  const resetSettings = useCallback(() => {
    setSettings(defaultDesktopSettings());
    setStartup(defaultWorkspaceStartupSettings());
  }, [setSettings, setStartup]);
  const refreshProfile = useCallback(() => {
    void loadProfile().then(setProfile).catch((error) => setSaveError(String(error)));
  }, []);

  return {
    profile,
    ready,
    settings: profile.settings.desktop,
    startup: profile.settings.workspaces,
    recents: profile.recents,
    sidebarWidth: profile.sidebarWidth,
    diagnostics: profile.settings.diagnostics,
    saveError,
    clearSaveError: () => setSaveError(null),
    setSettings,
    setStartup,
    rememberWorkspace,
    clearRecents,
    removeRecent,
    resetSettings,
    refreshProfile,
  };
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
