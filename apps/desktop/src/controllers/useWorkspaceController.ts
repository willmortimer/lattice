import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { demoSnapshot, inBrowser } from "../demo";
import { bridgeWorkspacePath, hasTauri, inBridgeMode, invoke } from "../lib/ipc";
import { listTemplates, provisionWorkspace, type TemplateDescriptor } from "../lib/templates";
import {
  loadWorkspaceUiSession,
  saveWorkspaceUiSession,
  type WorkspaceUiSession,
} from "../lib/workspaceUiSession";
import { openWorkspaceById } from "../lib/workspaceCatalog";
import { useWorkspaceUiSessionStore } from "../shell/workspaceUiSessionStore";
import { refreshResourceCatalog } from "../lib/resourceLinks";
import type { Resource, WorkspaceChangeEvent, WorkspaceSnapshot } from "../types";
import { workspaceUnavailableState } from "./workspacePolicy";

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
  getWorkspaceUiSession: () => Omit<WorkspaceUiSession, "workspaceId">;
  restoreWorkspaceUiSession: (
    session: WorkspaceUiSession,
    snapshot: WorkspaceSnapshot,
  ) => void | Promise<void>;
  onAdopt: (snapshot: WorkspaceSnapshot) => void | Promise<void>;
  onWorkspaceUnavailable: (root: string) => void | Promise<void>;
  /** Opens a seeded resource after create_workspace when the template sets openOnCreate. */
  openResource: (resource: Resource) => void | Promise<void>;
}

export interface WorkspaceController {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  setSnapshot: Dispatch<SetStateAction<WorkspaceSnapshot | null>>;
  workspacesDir: string | null;
  templates: TemplateDescriptor[];
  adoptWorkspace: (snapshot: WorkspaceSnapshot) => Promise<void>;
  handleWorkspaceChanged: (event: WorkspaceChangeEvent) => Promise<void>;
  refreshResources: () => Promise<void>;
  handleGetStarted: () => Promise<void>;
  handleOpenWorkspace: () => Promise<void>;
  openRecent: (root: string) => Promise<void>;
  openWorkspaceById: (workspaceId: string) => Promise<void>;
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

/** Owns workspace identity, profile-backed startup/session state, and the
 * native watcher/index lifecycle. Other controllers receive lifecycle
 * callbacks instead of reaching back into this hook. */
export function useWorkspaceController(options: WorkspaceControllerOptions): WorkspaceController {
  const {
    initialSnapshot, profile, profileReady, startup, recents, demoStartEmpty,
    setError, setBusy, setStatusToast, setNewWorkspaceOpen, rememberWorkspace,
    removeRecent, refreshProfile, getWorkspaceUiSession, restoreWorkspaceUiSession, onAdopt,
    onWorkspaceUnavailable, openResource,
  } = options;
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(initialSnapshot);
  const [workspacesDir, setWorkspacesDir] = useState<string | null>(null);
  const [templates, setTemplates] = useState<TemplateDescriptor[]>([]);
  const snapshotRef = useRef(snapshot);
  const startupAttemptedRef = useRef(false);
  const watchingRootRef = useRef<string | null>(null);
  const sessionRestoredWorkspaceIdRef = useRef<string | null>(null);

  const persistOutgoingWorkspaceUiSession = useCallback(async () => {
    const current = snapshotRef.current;
    if (!current || sessionRestoredWorkspaceIdRef.current !== current.id) return;
    const session: WorkspaceUiSession = {
      workspaceId: current.id,
      ...getWorkspaceUiSession(),
    };
    useWorkspaceUiSessionStore.getState().setWarmSession(session);
    await saveWorkspaceUiSession(session).catch(() => undefined);
  }, [getWorkspaceUiSession]);

  useEffect(() => {
    snapshotRef.current = snapshot;
  }, [snapshot]);

  useEffect(() => {
    void listTemplates().then(setTemplates).catch((error: unknown) => setError(String(error)));
  }, [setError]);

  const stopWatching = useCallback(async () => {
    if (!hasTauri || !watchingRootRef.current) return;
    watchingRootRef.current = null;
    await invoke("stop_watching").catch(() => undefined);
  }, []);

  useEffect(() => () => {
    void stopWatching();
  }, [stopWatching]);

  const adoptWorkspace = useCallback(async (next: WorkspaceSnapshot) => {
    await persistOutgoingWorkspaceUiSession();
    if (watchingRootRef.current && watchingRootRef.current !== next.root) await stopWatching();
    snapshotRef.current = next;
    sessionRestoredWorkspaceIdRef.current = null;
    useWorkspaceUiSessionStore.getState().setActiveWorkspaceId(next.id);
    setSnapshot(next);
    await onAdopt(next);
    rememberWorkspace(next);
    if (hasTauri) {
      watchingRootRef.current = next.root;
      void invoke("start_watching", { root: next.root }).catch((error: unknown) => {
        if (watchingRootRef.current === next.root) watchingRootRef.current = null;
        console.error("failed to start workspace watcher:", error);
      });
      // Do not await catalog refresh on the adopt critical path — wiki-link
      // resolution can catch up after shell chrome paints (warm-shell budget).
      void refreshResourceCatalog(next.root).catch(() => undefined);
    }
    if (hasTauri || inBridgeMode) {
      void invoke("rebuild_index", { root: next.root }).catch(() => undefined);
    }
  }, [onAdopt, persistOutgoingWorkspaceUiSession, rememberWorkspace, stopWatching]);

  const refreshResources = useCallback(async () => {
    const root = snapshotRef.current?.root;
    if (!root) return;
    try {
      const resources = await invoke<Resource[]>("list_resources", { root });
      setSnapshot((prev) => (prev ? { ...prev, resources } : prev));
      if (hasTauri) await refreshResourceCatalog(root);
    } catch {
      // A scan can briefly observe a file mid-write or a workspace closing.
    }
  }, []);

  const handleWorkspaceChanged = useCallback(async (event: WorkspaceChangeEvent) => {
    const root = snapshotRef.current?.root;
    if (!root) return;
    if (event.type === "workspace-unavailable" || (event.type === "deleted" && event.path === "lattice.yaml")) {
      // Atomic manifest replacement may briefly look like deletion. Reopen it
      // first; only reset after the watcher confirms the workspace is gone.
      if (event.type === "deleted") {
        try {
          await adoptWorkspace(await invoke<WorkspaceSnapshot>("open_workspace", { path: root }));
          return;
        } catch {
          // Continue with the honest unavailable state below.
        }
      }
      await stopWatching();
      const reset = workspaceUnavailableState(root);
      snapshotRef.current = reset.snapshot;
      sessionRestoredWorkspaceIdRef.current = null;
      useWorkspaceUiSessionStore.getState().setActiveWorkspaceId(null);
      setSnapshot(reset.snapshot);
      await onWorkspaceUnavailable(root);
      await refreshProfile();
    }
  }, [adoptWorkspace, onWorkspaceUnavailable, refreshProfile, stopWatching]);

  const handleOpenWorkspace = useCallback(async () => {
    setError(null);
    if (inBridgeMode) {
      const path = bridgeWorkspacePath;
      if (!path) {
        setError(
          "Bridge mode has no native folder dialog. Set VITE_LATTICE_WORKSPACE or use Get started (ensure_home).",
        );
        return;
      }
      setBusy(true);
      try {
        await adoptWorkspace(await invoke<WorkspaceSnapshot>("open_workspace", { path }));
      } catch (error) {
        setError(String(error));
      } finally {
        setBusy(false);
      }
      return;
    }
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

  type EnsureHomeResult = {
    workspaces: string;
    default_workspace: WorkspaceSnapshot | null;
    diagnostics: Array<{ message: string }>;
    demoReset?: boolean;
  };

  const provisionDefaultHome = useCallback(async () => {
    const home = await invoke<EnsureHomeResult>("ensure_home");
    setWorkspacesDir(home.workspaces);
    if (home.default_workspace) {
      await adoptWorkspace(home.default_workspace);
      if (home.diagnostics.length > 0) {
        setStatusToast(home.diagnostics.map((item) => item.message).join(" "));
      }
    } else {
      setError("Lattice home is ready, but no default workspace was found.");
    }
    return home;
  }, [adoptWorkspace, setError, setStatusToast]);

  const handleGetStarted = useCallback(async () => {
    setError(null);
    if (inBrowser) {
      await adoptWorkspace(demoSnapshot);
      return;
    }
    setBusy(true);
    try {
      await provisionDefaultHome();
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [adoptWorkspace, provisionDefaultHome, setBusy, setError]);

  const openRecent = useCallback(async (root: string) => {
    setError(null);
    if (inBrowser) {
      await adoptWorkspace(demoSnapshot);
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

  const openWorkspaceByIdHandler = useCallback(async (workspaceId: string) => {
    setError(null);
    if (inBrowser) {
      await adoptWorkspace(demoSnapshot);
      return;
    }
    setBusy(true);
    try {
      await adoptWorkspace(await openWorkspaceById(workspaceId));
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [adoptWorkspace, setBusy, setError]);

  const handleCreateWorkspace = useCallback(async (args: {
    path: string; title: string; template: string; setDefault: boolean; initializeExisting: boolean;
  }) => {
    setError(null);
    if (!inBrowser) setBusy(true);
    try {
      const outcome = await provisionWorkspace(args);
      await adoptWorkspace(outcome.workspace);
      await refreshProfile();
      if (outcome.diagnostics.length > 0) setStatusToast(outcome.diagnostics.map((item) => item.message).join(" "));
      else if (!inBrowser) setStatusToast(`Created ${outcome.workspace.title}`);
      setNewWorkspaceOpen(false);
      const pathToOpen = templates.find((template) => template.id === args.template)?.openOnCreate;
      if (pathToOpen) {
        const resource = outcome.workspace.resources.find((entry) => entry.path === pathToOpen);
        if (resource) await openResource(resource);
      }
    } catch (error) {
      setError(String(error));
    } finally {
      if (!inBrowser) setBusy(false);
    }
  }, [adoptWorkspace, openResource, refreshProfile, setBusy, setError, setNewWorkspaceOpen, setStatusToast, templates]);

  const openNewWorkspaceDialog = useCallback(async () => {
    setError(null);
    if (hasTauri && !profile.workspacesDirectory) return;
    setNewWorkspaceOpen(true);
  }, [profile.workspacesDirectory, setError, setNewWorkspaceOpen]);

  const pickWorkspaceFolder = useCallback(async () => {
    if (inBridgeMode) return null;
    const path = await open({ directory: true, multiple: false, title: "Choose workspace destination" });
    return typeof path === "string" ? path : null;
  }, []);

  useEffect(() => {
    if (demoStartEmpty || snapshot || !profileReady || startupAttemptedRef.current) return;

    if (inBridgeMode) {
      if (!bridgeWorkspacePath) return;
      startupAttemptedRef.current = true;
      let cancelled = false;
      void (async () => {
        try {
          const next = await invoke<WorkspaceSnapshot>("open_workspace", {
            path: bridgeWorkspacePath,
          });
          if (!cancelled) await adoptWorkspace(next);
        } catch (error) {
          if (!cancelled) setError(String(error));
        }
      })();
      return () => {
        cancelled = true;
      };
    }

    if (inBrowser || !hasTauri) return;
    startupAttemptedRef.current = true;
    let cancelled = false;
    void (async () => {
      // Always run ensure_home first so LATTICE_DEV_RESET_DEMO can re-seed First Look
      // before we try reopen-last paths (those would otherwise skip the reset).
      let home: EnsureHomeResult | null = null;
      try {
        home = await invoke<EnsureHomeResult>("ensure_home");
        if (cancelled) return;
        setWorkspacesDir(home.workspaces);
      } catch (error) {
        if (!cancelled) setError(String(error));
        return;
      }

      if (home.demoReset && home.default_workspace) {
        await adoptWorkspace(home.default_workspace);
        if (home.diagnostics.length > 0) {
          setStatusToast(home.diagnostics.map((item) => item.message).join(" "));
        }
        return;
      }

      const candidates = [
        ...(startup.reopenLastWorkspace ? recents.map((recent) => recent.root) : []),
        profile.effectiveDefaultWorkspace,
        home.default_workspace?.root ?? null,
      ].filter((path, index, all): path is string => Boolean(path) && all.indexOf(path) === index);

      if (candidates.length === 0) {
        if (home.default_workspace) {
          await adoptWorkspace(home.default_workspace);
        } else {
          setError("Lattice home is ready, but no default workspace was found.");
        }
        return;
      }

      for (const path of candidates) {
        try {
          const next = await invoke<WorkspaceSnapshot>("open_workspace", { path });
          if (!cancelled) await adoptWorkspace(next);
          return;
        } catch {
          if (recents.some((recent) => recent.root === path)) removeRecent(path);
        }
      }
      if (!cancelled && home.default_workspace) {
        await adoptWorkspace(home.default_workspace);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [adoptWorkspace, demoStartEmpty, profile.effectiveDefaultWorkspace, profileReady, recents, removeRecent, setError, setStatusToast, snapshot, startup.reopenLastWorkspace]);

  useEffect(() => {
    if (!snapshot || sessionRestoredWorkspaceIdRef.current === snapshot.id) return;
    sessionRestoredWorkspaceIdRef.current = snapshot.id;
    if (!startup.restoreSession) return;

    const warm = useWorkspaceUiSessionStore.getState().getWarmSession(snapshot.id);
    if (warm) {
      void restoreWorkspaceUiSession(warm, snapshot);
      return;
    }

    void loadWorkspaceUiSession(snapshot.id, snapshot.root)
      .then((stored) => {
        if (stored) void restoreWorkspaceUiSession(stored, snapshot);
      })
      .catch(() => undefined);
  }, [restoreWorkspaceUiSession, snapshot, startup.restoreSession]);

  useEffect(() => {
    if (!snapshot || sessionRestoredWorkspaceIdRef.current !== snapshot.id) return;
    const timer = window.setTimeout(() => {
      const session: WorkspaceUiSession = {
        workspaceId: snapshot.id,
        ...getWorkspaceUiSession(),
      };
      useWorkspaceUiSessionStore.getState().setWarmSession(session);
      void saveWorkspaceUiSession(session).catch(() => undefined);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [getWorkspaceUiSession, snapshot, snapshot?.id]);

  return {
    snapshot, snapshotRef, setSnapshot, workspacesDir, templates, adoptWorkspace,
    handleWorkspaceChanged, refreshResources, handleGetStarted, handleOpenWorkspace,
    openRecent, openWorkspaceById: openWorkspaceByIdHandler, handleCreateWorkspace, openNewWorkspaceDialog, pickWorkspaceFolder,
  };
}
