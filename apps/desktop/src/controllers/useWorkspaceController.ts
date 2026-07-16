import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { demoSnapshot, inBrowser } from "../demo";
import { listTemplates, provisionWorkspace, type TemplateDescriptor } from "../lib/templates";
import { refreshResourceCatalog } from "../lib/resourceLinks";
import type { WorkspaceSnapshot } from "../types";

type Profile = ReturnType<typeof import("../settings/model").useAppSettings>["profile"];
type Startup = ReturnType<typeof import("../settings/model").useAppSettings>["startup"];
type Recent = ReturnType<typeof import("../settings/model").useAppSettings>["recents"][number];

export interface WorkspaceControllerOptions {
  initialSnapshot: WorkspaceSnapshot | null;
  profile: Profile;
  profileReady: boolean;
  startup: Startup;
  recents: Recent[];
  demoStartEmpty: boolean;
  setError: (message: string | null) => void;
  setBusy: (busy: boolean) => void;
  setStatusToast: (message: string | null) => void;
  setNewWorkspaceOpen: (open: boolean) => void;
  rememberWorkspace: (snapshot: WorkspaceSnapshot) => void;
  removeRecent: (root: string) => void;
  refreshProfile: () => void | Promise<void>;
  onAdopt: (snapshot: WorkspaceSnapshot) => void | Promise<void>;
}

export interface WorkspaceController {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  setSnapshot: Dispatch<SetStateAction<WorkspaceSnapshot | null>>;
  workspacesDir: string | null;
  templates: TemplateDescriptor[];
  adoptWorkspace: (snapshot: WorkspaceSnapshot) => Promise<void>;
  handleGetStarted: () => Promise<void>;
  handleOpenWorkspace: () => Promise<void>;
  openRecent: (root: string) => Promise<void>;
  handleCreateWorkspace: (args: {
    path: string;
    title: string;
    template: string;
    setDefault: boolean;
    initializeExisting: boolean;
  }) => Promise<void>;
  openNewWorkspaceDialog: () => Promise<void>;
  pickWorkspaceFolder: () => Promise<string | null>;
}

/** Owns workspace identity, profile-backed startup, adoption, and native
 * watcher/index lifecycle. Resource/UI cleanup is injected through onAdopt so
 * this controller does not depend on the resource controller. */
export function useWorkspaceController(options: WorkspaceControllerOptions): WorkspaceController {
  const {
    initialSnapshot, profile, profileReady, startup, recents, demoStartEmpty,
    setError, setBusy, setStatusToast, setNewWorkspaceOpen, rememberWorkspace,
    removeRecent, refreshProfile, onAdopt,
  } = options;
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(initialSnapshot);
  const [workspacesDir, setWorkspacesDir] = useState<string | null>(null);
  const [templates, setTemplates] = useState<TemplateDescriptor[]>([]);
  const snapshotRef = useRef(snapshot);
  const startupAttemptedRef = useRef(false);
  useEffect(() => {
    snapshotRef.current = snapshot;
  }, [snapshot]);

  useEffect(() => {
    void listTemplates().then(setTemplates).catch((error: unknown) => setError(String(error)));
  }, [setError]);

  const adoptWorkspace = useCallback(async (next: WorkspaceSnapshot) => {
    snapshotRef.current = next;
    setSnapshot(next);
    await onAdopt(next);
    rememberWorkspace(next);
    if (!inBrowser) {
      void invoke("start_watching", { root: next.root }).catch((error: unknown) => {
        console.error("failed to start workspace watcher:", error);
      });
      void invoke("rebuild_index", { root: next.root }).catch(() => undefined);
      await refreshResourceCatalog(next.root);
    }
  }, [onAdopt, rememberWorkspace]);

  const handleOpenWorkspace = useCallback(async () => {
    setError(null);
    const directory = await open({ directory: true, multiple: false, title: "Open Workspace" });
    if (!directory || Array.isArray(directory)) return;
    setBusy(true);
    try {
      await adoptWorkspace(await invoke<WorkspaceSnapshot>("open_workspace", { path: directory }));
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [adoptWorkspace, setBusy, setError]);

  const handleGetStarted = useCallback(async () => {
    setError(null);
    if (inBrowser) {
      setSnapshot(demoSnapshot);
      snapshotRef.current = demoSnapshot;
      rememberWorkspace(demoSnapshot);
      return;
    }
    setBusy(true);
    try {
      const home = await invoke<{ workspaces: string; default_workspace: WorkspaceSnapshot | null; diagnostics: Array<{ message: string }> }>("ensure_home");
      setWorkspacesDir(home.workspaces);
      if (home.default_workspace) {
        await adoptWorkspace(home.default_workspace);
        if (home.diagnostics.length > 0) setStatusToast(home.diagnostics.map((item) => item.message).join(" "));
      } else {
        setError("Lattice home is ready, but no default workspace was found.");
      }
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [adoptWorkspace, rememberWorkspace, setBusy, setError, setStatusToast]);

  const openRecent = useCallback(async (root: string) => {
    setError(null);
    if (inBrowser) {
      setSnapshot(demoSnapshot);
      snapshotRef.current = demoSnapshot;
      return;
    }
    setBusy(true);
    try {
      await adoptWorkspace(await invoke<WorkspaceSnapshot>("open_workspace", { path: root }));
    } catch (error) {
      removeRecent(root);
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [adoptWorkspace, removeRecent, setBusy, setError]);

  const handleCreateWorkspace = useCallback(async (args: {
    path: string; title: string; template: string; setDefault: boolean; initializeExisting: boolean;
  }) => {
    setError(null);
    if (!inBrowser) setBusy(true);
    try {
      const outcome = await provisionWorkspace(args);
      await adoptWorkspace(outcome.workspace);
      refreshProfile();
      if (outcome.diagnostics.length > 0) setStatusToast(outcome.diagnostics.map((item) => item.message).join(" "));
      else if (!inBrowser) setStatusToast(`Created ${outcome.workspace.title}`);
      setNewWorkspaceOpen(false);
    } catch (error) {
      setError(String(error));
    } finally {
      if (!inBrowser) setBusy(false);
    }
  }, [adoptWorkspace, refreshProfile, setBusy, setError, setNewWorkspaceOpen, setStatusToast]);

  const openNewWorkspaceDialog = useCallback(async () => {
    setError(null);
    if (!inBrowser && !profile.workspacesDirectory) return;
    setNewWorkspaceOpen(true);
  }, [profile.workspacesDirectory, setError, setNewWorkspaceOpen]);

  const pickWorkspaceFolder = useCallback(async () => {
    const path = await open({ directory: true, multiple: false, title: "Choose workspace destination" });
    return typeof path === "string" ? path : null;
  }, []);

  useEffect(() => {
    if (inBrowser || demoStartEmpty || snapshot || !profileReady || startupAttemptedRef.current) return;
    startupAttemptedRef.current = true;
    const candidates = [
      ...(startup.reopenLastWorkspace ? recents.map((recent) => recent.root) : []),
      profile.effectiveDefaultWorkspace,
    ].filter((path, index, all): path is string => Boolean(path) && all.indexOf(path) === index);
    if (candidates.length === 0) return;
    let cancelled = false;
    void (async () => {
      for (const path of candidates) {
        try {
          const next = await invoke<WorkspaceSnapshot>("open_workspace", { path });
          if (!cancelled) await adoptWorkspace(next);
          return;
        } catch {
          if (recents.some((recent) => recent.root === path)) removeRecent(path);
        }
      }
    })();
    return () => { cancelled = true; };
  }, [adoptWorkspace, demoStartEmpty, profile.effectiveDefaultWorkspace, profileReady, recents, removeRecent, snapshot, startup.reopenLastWorkspace]);

  return {
    snapshot, snapshotRef, setSnapshot, workspacesDir, templates, adoptWorkspace,
    handleGetStarted, handleOpenWorkspace, openRecent, handleCreateWorkspace,
    openNewWorkspaceDialog, pickWorkspaceFolder,
  };
}
