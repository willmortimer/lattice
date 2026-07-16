import { demoSnapshot, demoStartEmpty, inBrowser } from "../demo";
import type { DataAppSnapshot } from "../data/types";
import { isUnsaved, type SaveState } from "../editor/saveState";
import { createDemoPageIO } from "../editor/pageIO";
import type { PageEditorHandle } from "../editor/PageEditor";
import { loadSession, saveSession, saveSidebarWidth } from "../lib/profile";
import { demoLinkTargets, refreshResourceCatalog, resolveResourceLink, type ResourceLinkTarget } from "../lib/resourceLinks";
import { updateWorkspaceManifest } from "../lib/workspace";
import { installNativeContextMenus } from "../lib/nativeMenus";
import { fileTimestamp, quickNotePath } from "../lib/timestamp";
import { QUICK_NOTE_SHORTCUT, showQuickNote } from "../quickNoteWindow";
import { applyResolvedTheme, loadThemeCatalog, setAppearanceMode, setFixedTheme, startThemeWatch, type ThemeCatalogPayload, type ThemeSummaryPayload } from "../theme";
import type { Resource, WorkspaceChangeEvent, WorkspaceSnapshot } from "../types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { matchesKeybinding, useAppSettings } from "../settings/model";
import { useNavigationController } from "./useNavigationController";
import { useResourceController } from "./useResourceController";
import { useResourceReconciliation } from "./useResourceReconciliation";
import { useWorkspaceController } from "./useWorkspaceController";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import type { PaletteItem } from "../CommandPalette";
interface ExternalConflict {
  path: string;
}

/** `Notes/Idea.md` -> `Notes`; a top-level path (no `/`) -> `""` (the workspace root). */
function dirnameOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash >= 0 ? path.slice(0, slash) : "";
}

/** Build the "keep both" sibling path: `Notes/Idea.md` -> `Notes/Idea (conflict 2026-07-15).md`. */
function conflictSiblingPath(path: string): string {
  const slash = path.lastIndexOf("/");
  const dir = slash >= 0 ? path.slice(0, slash + 1) : "";
  const base = slash >= 0 ? path.slice(slash + 1) : path;
  const dot = base.lastIndexOf(".");
  const stem = dot > 0 ? base.slice(0, dot) : base;
  const ext = dot > 0 ? base.slice(dot) : "";
  const date = new Date().toISOString().slice(0, 10);
  return `${dir}${stem} (conflict ${date})${ext}`;
}

type ActivityArea = "home" | "files" | "search" | "quick-note" | "settings";

export function fileTitle(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.(md|canvas|pdf|png|jpe?g)$/i, "").replace(/\.data$/i, "");
}

function renamedPath(path: string, title: string): string {
  const slash = path.lastIndexOf("/");
  const dir = slash >= 0 ? path.slice(0, slash + 1) : "";
  const base = slash >= 0 ? path.slice(slash + 1) : path;
  const dataSuffix = base.endsWith(".data") ? ".data" : "";
  const dot = dataSuffix ? -1 : base.lastIndexOf(".");
  const extension = dataSuffix || (dot > 0 ? base.slice(dot) : "");
  return `${dir}${title.trim()}${extension}`;
}

/** Derive a SQL table name from a human label (palette "New table…"). */
function tableNameFromLabel(label: string): string {
  let name = label
    .trim()
    .replace(/\.data$/i, "")
    .toLowerCase()
    .replace(/[^a-z0-9_]+/g, "_")
    .replace(/^_+|_+$/g, "");
  if (!name || /^\d/.test(name)) {
    name = `t_${name || "table"}`;
  }
  return name;
}

/** `Tasks` -> `Tasks.data` */
function dataPackagePath(label: string): string {
  const trimmed = label.trim().replace(/\.data$/i, "");
  return `${trimmed}.data`;
}


export function useDesktopController() {
  const {
    profile,
    ready: profileReady,
    settings,
    startup,
    recents,
    diagnostics: profileDiagnostics,
    saveError: profileSaveError,
    setSettings,
    setStartup,
    rememberWorkspace,
    clearRecents,
    removeRecent,
    refreshProfile,
    resetSettings,
  } = useAppSettings();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });
  const [externalConflict, setExternalConflict] = useState<ExternalConflict | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [newWorkspaceOpen, setNewWorkspaceOpen] = useState(false);
  const [statusToast, setStatusToast] = useState<string | null>(null);
  const [runtimeNotice, setRuntimeNotice] = useState<{
    code: string; title: string; message: string; path: string | null;
  } | null>(null);
  const [dismissedNoticeCodes, setDismissedNoticeCodes] = useState<string[]>([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [searchPaneOpen, setSearchPaneOpen] = useState(false);
  const [themeCatalog, setThemeCatalog] = useState<ThemeCatalogPayload | null>(null);
  const [activityArea, setActivityArea] = useState<ActivityArea>("files");
  const [sidebarWidth, setSidebarWidth] = useState(272);
  const [revealPath, setRevealPath] = useState<string | null>(null);
  const [linkPicker, setLinkPicker] = useState<{
    query: string; candidates: ResourceLinkTarget[];
  } | null>(null);
  const navigationController = useNavigationController();
  const { state: navigation } = navigationController;
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const sessionRestoredRootRef = useRef<string | null>(null);
  const workspaceSettingsTimerRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const currentPageRevisionRef = useRef<string | null>(null);
  const pageEditorRef = useRef<PageEditorHandle>(null);
  const resourceResetRef = useRef<() => void>(() => undefined);

  useEffect(() => installNativeContextMenus(() => settingsRef.current.diagnostics.nativeContextMenus), []);
  useEffect(() => {
    document.documentElement.dataset.motion = settings.performance.reducedMotion;
  }, [settings.performance.reducedMotion]);
  useEffect(() => () => {
    if (workspaceSettingsTimerRef.current) window.clearTimeout(workspaceSettingsTimerRef.current);
  }, []);
  useEffect(() => {
    if (profile.sidebarWidth && profile.sidebarWidth >= 210 && profile.sidebarWidth <= 480) {
      setSidebarWidth(profile.sidebarWidth);
    }
  }, [profile.sidebarWidth]);
  useEffect(() => {
    const messages = [
      ...profileDiagnostics.map((diagnostic) => `${diagnostic.path}: ${diagnostic.message}`),
      ...(profileSaveError ? [profileSaveError] : []),
    ];
    if (messages.length > 0) setError(messages.join("\n"));
  }, [profileDiagnostics, profileSaveError]);

  const onAdopt = useCallback(async () => {
    resourceResetRef.current();
    setSaveState({ status: "idle" });
    setExternalConflict(null);
    setRuntimeNotice(null);
    navigationController.reset();
    setActivityArea("home");
    sessionRestoredRootRef.current = null;
  }, [navigationController]);

  const workspaceController = useWorkspaceController({
    initialSnapshot: inBrowser && !demoStartEmpty ? demoSnapshot : null,
    profile,
    profileReady,
    startup,
    recents,
    demoStartEmpty,
    setError,
    setBusy,
    setStatusToast,
    setNewWorkspaceOpen,
    rememberWorkspace,
    removeRecent,
    refreshProfile,
    onAdopt,
  });
  const { snapshot, snapshotRef, setSnapshot, workspacesDir, templates, adoptWorkspace,
    handleGetStarted, handleOpenWorkspace, openRecent, handleCreateWorkspace,
    openNewWorkspaceDialog, pickWorkspaceFolder } = workspaceController;
  const assetRoot = inBrowser ? null : snapshot?.root ?? null;
  const wikiTargets = useMemo(() => demoLinkTargets(snapshot?.resources ?? []), [snapshot?.resources]);
  const hasCapability = useCallback(
    (capability: string) => capability === "pages" || Boolean(snapshot?.capabilities.includes(capability)),
    [snapshot?.capabilities],
  );

  const resourceController = useResourceController({
    snapshot,
    snapshotRef,
    settings,
    hasCapability,
    onError: setError,
    onBusy: setBusy,
    onActivity: setActivityArea,
    onTitle: (title) => { setTitleDraft(title); setEditingTitle(false); },
    onResetSelection: () => {
      setExternalConflict(null);
      setReloadToken(0);
      currentPageRevisionRef.current = null;
    },
    onRecordNavigation: navigationController.record,
    onPageReady: () => setSaveState({ status: "idle" }),
  });
  resourceResetRef.current = resourceController.resetResources;
  const { selected, setSelected, session, setSession, openTabs, setOpenTabs, handleSelect } = resourceController;
  const page = session?.kind === "page" ? session : null;
  const pageRef = useRef(page);
  useEffect(() => { pageRef.current = page; }, [page]);
  const saveStateRef = useRef(saveState);
  useEffect(() => { saveStateRef.current = saveState; }, [saveState]);
  const selectedRef = useRef(selected);
  useEffect(() => { selectedRef.current = selected; }, [selected]);
  const profileNotices = [runtimeNotice, ...profile.notices]
    .filter((notice): notice is NonNullable<typeof notice> => notice !== null)
    .filter((notice) => !dismissedNoticeCodes.includes(notice.code));
  const applyThemeCatalog = useCallback((catalog: ThemeCatalogPayload) => {
    setThemeCatalog(catalog);
    applyResolvedTheme(catalog.resolved);
    const diags = [...catalog.diagnostics, ...catalog.resolved.diagnostics].filter(
      (diagnostic, index, all) =>
        all.findIndex(
          (candidate) =>
            candidate.path === diagnostic.path && candidate.message === diagnostic.message,
        ) === index,
    );
    if (diags.length > 0) {
      setError(diags.map((d) => `${d.path}: ${d.message}`).join("\n"));
    }
  }, []);

  const reloadTheme = useCallback(async () => {
    try {
      const catalog = await loadThemeCatalog(snapshotRef.current?.root);
      applyThemeCatalog(catalog);
    } catch (err) {
      setError(String(err));
    }
  }, [applyThemeCatalog]);

  // Initial theme resolve + re-apply when the open workspace changes.
  useEffect(() => {
    void reloadTheme();
  }, [reloadTheme, snapshot?.root]);

  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      stop = await startThemeWatch(snapshot?.root ?? null, () => {
        if (!cancelled) void reloadTheme();
      });
    })();
    return () => {
      cancelled = true;
      stop?.();
    };
  }, [snapshot?.root, reloadTheme]);

  const refreshSidebar = useCallback(async () => {
    const root = snapshotRef.current?.root;
    if (!root) return;
    try {
      const resources = await invoke<Resource[]>("list_resources", { root });
      setSnapshot((prev) => (prev ? { ...prev, resources } : prev));
      await refreshResourceCatalog(root);
    } catch {
      // Transient (e.g. a scan mid-write, or the workspace just closed);
      // the next event or a manual reopen catches the list up.
    }
  }, []);

  const reloadPageFromDisk = useCallback(async () => {
    const current = pageRef.current;
    if (!current) return;
    try {
      const { raw, revision } = await current.io.load();
      setSession((prev) =>
        prev?.kind === "page" ? { ...prev, content: raw, revision } : prev,
      );
      setReloadToken((t) => t + 1);
      setSaveState({ status: "idle" });
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleWorkspaceChanged = useCallback(
    async (event: WorkspaceChangeEvent) => {
      const root = snapshotRef.current?.root;
      if (
        event.type === "workspace-unavailable" ||
        (event.type === "deleted" && event.path === "lattice.yaml")
      ) {
        if (!root) return;
        // A genuine manifest/root deletion invalidates the workspace as a
        // command boundary. Keep its path for repair, but do not recreate it.
        if (event.type === "deleted") {
          try {
            const reopened = await invoke<WorkspaceSnapshot>("open_workspace", { path: root });
            setSnapshot(reopened);
            return;
          } catch {
            // It remains unavailable after watcher debounce: genuine loss.
          }
        }
        invoke("stop_watching").catch(() => undefined);
        snapshotRef.current = null;
        setSnapshot(null);
        setSelected(null);
        setSession(null);
        setOpenTabs([]);
        setExternalConflict(null);
        setRuntimeNotice({
          code: "open-workspace-unavailable",
          title: "Workspace unavailable",
          message:
            "The open workspace was moved or deleted outside Lattice. It was closed without recreating any content; create a workspace or open its new location.",
          path: root,
        });
        void refreshProfile();
        return;
      }

      await refreshSidebar();

      const current = pageRef.current;

      if (event.type === "renamed") {
        if (current && event.from === current.resource.path) {
          setError(`"${event.from}" was renamed to "${event.to}" outside Lattice.`);
          setSession(null);
          setSelected(null);
          setExternalConflict(null);
        }
        const currentSelection = selectedRef.current;
        if (currentSelection?.path === event.from) {
          setError(`"${event.from}" was renamed to "${event.to}" outside Lattice.`);
          setSelected(null);
          setSession(null);
          setOpenTabs((tabs) => tabs.filter((tab) => tab.path !== event.from));
        }
        return;
      }

      if (event.type === "deleted") {
        if (current && event.path === current.resource.path) {
          // Defensive second check: macOS atomic replacement can surface a
          // transient remove event. If the page is readable now, reconcile it
          // as a modification instead of ejecting the editor.
          try {
            const disk = await current.io.load();
            if (disk.revision === currentPageRevisionRef.current) return;
            if (isUnsaved(saveStateRef.current)) {
              setExternalConflict({ path: event.path });
            } else {
              setSession((previous) =>
                previous?.kind === "page"
                  ? { ...previous, content: disk.raw, revision: disk.revision }
                  : previous,
              );
              setReloadToken((token) => token + 1);
              setSaveState({ status: "idle" });
            }
            return;
          } catch {
            // A genuine deletion remains unreadable and falls through to the
            // existing close-and-report behavior.
          }
          setError(`"${event.path}" was deleted outside Lattice.`);
          setSession(null);
          setSelected(null);
          setExternalConflict(null);
        }
        const currentSelection = selectedRef.current;
        if (
          currentSelection &&
          (currentSelection.path === event.path ||
            currentSelection.path.startsWith(`${event.path}/`))
        ) {
          setError(`"${event.path}" was deleted outside Lattice.`);
          setSelected(null);
          setSession(null);
          setOpenTabs((tabs) =>
            tabs.filter(
              (tab) => tab.path !== event.path && !tab.path.startsWith(`${event.path}/`),
            ),
          );
        }
        return;
      }

      if (!current) return;
      if (event.path !== current.resource.path) return;

      if (event.revision === currentPageRevisionRef.current) {
        // Echo of a save this window already knows about (our own
        // autosave, or a conflict resolution) — nothing to reconcile.
        return;
      }

      if (isUnsaved(saveStateRef.current)) {
        setExternalConflict({ path: event.path });
      } else {
        await reloadPageFromDisk();
      }
    },
    [refreshProfile, refreshSidebar, reloadPageFromDisk],
  );

  useResourceReconciliation(handleWorkspaceChanged);

  async function handleKeepIncoming() {
    await reloadPageFromDisk();
    setExternalConflict(null);
  }

  async function handleKeepLocal() {
    const current = pageRef.current;
    const editorHandle = pageEditorRef.current;
    if (!current || !editorHandle) return;
    const localRaw = editorHandle.getRaw();
    try {
      // The incoming disk state becomes the base we intentionally overwrite
      // with local content — "keep local" is a deliberate clobber, not a
      // merge, so this is expected to succeed rather than hit STALE_REVISION.
      const disk = await current.io.load();
      const newRevision = await current.io.save(localRaw, disk.revision);
      setSession((prev) =>
        prev?.kind === "page" ? { ...prev, content: localRaw, revision: newRevision } : prev,
      );
      setReloadToken((t) => t + 1);
      setSaveState({ status: "idle" });
      setExternalConflict(null);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleKeepBoth() {
    const current = pageRef.current;
    const editorHandle = pageEditorRef.current;
    const root = snapshotRef.current?.root;
    if (!current || !editorHandle || !root) return;
    const localRaw = editorHandle.getRaw();
    const siblingPath = conflictSiblingPath(current.resource.path);
    try {
      await invoke("create_page", { root, relPath: siblingPath, content: localRaw });
      await reloadPageFromDisk();
      setExternalConflict(null);
      await refreshSidebar();
    } catch (err) {
      setError(String(err));
    }
  }

  /**
   * Create a new blank page at `relPath` and open it — shared by the
   * command palette's "New page" and the Cmd/Ctrl+N quick-note shortcut.
   * Unlike `handleSelect`, the demo shell gets a genuinely blank page
   * rather than a fixture page, since this is meant to look like
   * a freshly created note.
   */
  async function createAndOpenPage(relPath: string) {
    const resource: Resource = { path: relPath, kind: "page" };

    if (inBrowser) {
      setSnapshot((prev) => (prev ? { ...prev, resources: [...prev.resources, resource] } : prev));
      setSelected(resource);
      setError(null);
      setSession(null);
      setExternalConflict(null);
      setSaveState({ status: "idle" });
      setReloadToken((t) => t + 1);
      currentPageRevisionRef.current = "demo:0";
      setSession({ kind: "page", resource, content: "", revision: "demo:0", io: createDemoPageIO("") });
      return;
    }

    if (!snapshot) return;
    setBusy(true);
    try {
      await invoke("create_page", { root: snapshot.root, relPath, content: "" });
      await refreshSidebar();
      await handleSelect(resource);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  /** Open the dedicated capture window. Creation and saving still flow
   * through the same semantic commands as pages opened in the main shell. */
  function handleQuickNote() {
    if (inBrowser) {
      const path = quickNotePath(new Date(), snapshot?.defaults.quickNoteDirectory ?? "Inbox");
      setStatusToast(`Browser demo capture: ${path}`);
      window.setTimeout(() => setStatusToast(null), 2200);
      void createAndOpenPage(path);
      return;
    }
    void showQuickNote(snapshot?.root).catch((err) => setError(String(err)));
  }

  /** Command palette "New page": in the folder of the currently selected
   * resource, or the workspace root if nothing is selected. */
  function handleNewPage() {
    const dir = selected ? dirnameOf(selected.path) : "";
    const name = `Untitled ${fileTimestamp()}.md`;
    void createAndOpenPage(dir ? `${dir}/${name}` : name);
  }

  /** Command palette "Undo last change": reverts the workspace's most
   * recent transaction. Any files it touches are picked up by the
   * regular `workspace-changed` reconciliation, same as an external edit. */
  async function handleUndo() {
    if (!snapshot) return;
    try {
      await invoke<string | null>("undo_last", { root: snapshot.root });
      await refreshSidebar();
    } catch (err) {
      setError(String(err));
    }
  }

  /** Command palette "Import CSV…": create a `.data` package from a CSV file. */
  async function handleImportCsv() {
    if (inBrowser) {
      setError("CSV import is not available in the browser demo.");
      return;
    }
    if (!snapshot) return;

    const selected = await open({
      multiple: false,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!selected || typeof selected !== "string") return;

    const name = window.prompt("Package name", "Imported")?.trim();
    if (!name) return;

    setBusy(true);
    try {
      const [relPath, created] = await invoke<[string, DataAppSnapshot]>("import_csv_table", {
        root: snapshot.root,
        csvPath: selected,
        packageName: name,
        title: name.replace(/\.data$/i, ""),
        tableName: tableNameFromLabel(name),
      });
      await refreshSidebar();
      const resource: Resource = { path: relPath, kind: "data-app" };
      setSelected(resource);
      setSession({ kind: "data-app", resource, snapshot: created });
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  /** Command palette "New table…": create a `.data` package at the workspace root. */
  async function handleNewTable() {
    const name = window.prompt("New table name");
    if (!name?.trim()) return;

    const relPath = dataPackagePath(name);
    const title = name.trim().replace(/\.data$/i, "");
    const tableName = tableNameFromLabel(name);
    const resource: Resource = { path: relPath, kind: "data-app" };

    if (inBrowser) {
      if (snapshot?.resources.some((entry) => entry.path === relPath)) {
        setError(`${relPath} already exists`);
        return;
      }
      setSnapshot((prev) =>
        prev ? { ...prev, resources: [...prev.resources, resource] } : prev,
      );
      setSelected(resource);
      setSession(null);
      setExternalConflict(null);
      setError(null);
      setSession({
        kind: "data-app",
        resource,
        snapshot: {
          title,
          default_table: tableName,
          package_revision: "demo:0",
          columns: [{ name: "id", field_type: "text", sqlite_type: "TEXT" }],
          rows: [],
          available_views: ["All"],
          active_view: "All",
          filters: [],
        },
      });
      return;
    }

    if (!snapshot) return;
    setBusy(true);
    try {
      const created = await invoke<DataAppSnapshot>("create_table_package", {
        root: snapshot.root,
        relPath,
        title,
        tableName,
      });
      await refreshSidebar();
      setSelected(resource);
      setSession({ kind: "data-app", resource, snapshot: created });
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleImportEditorAsset(file: File): Promise<string> {
    const workspace = snapshotRef.current;
    const currentPage = pageRef.current;
    if (inBrowser || !workspace || !currentPage) {
      throw new Error("File import requires the native desktop workspace.");
    }

    const content = new Uint8Array(await file.arrayBuffer());
    const relativePath = await invoke<string>("create_asset", content, {
      headers: {
        "x-lattice-root": encodeURIComponent(workspace.root),
        "x-lattice-page-path": encodeURIComponent(currentPage.resource.path),
        "x-lattice-file-name": encodeURIComponent(file.name),
      },
    });
    await refreshSidebar();
    return relativePath;
  }

  /** Placeholder view's "Open externally" button, for resource kinds with
   * no built-in viewer (PDFs, images, and other binary embeds). */
  async function handleOpenExternally(resource: Resource) {
    if (!snapshot) return;
    try {
      await openPath(`${snapshot.root}/${resource.path}`);
    } catch (err) {
      setError(String(err));
    }
  }

  function navigateHistory(delta: -1 | 1) {
    const nextIndex = navigation.index + delta;
    if (nextIndex < 0 || nextIndex >= navigation.paths.length) return;
    const path = navigation.paths[nextIndex];
    const resource = snapshot?.resources.find((entry) => entry.path === path);
    if (!resource) return;
    navigationController.go(delta);
    void handleSelect(resource, { recordHistory: false });
  }

  function closeTab(path: string) {
    if (
      path === selected?.path &&
      isUnsaved(saveState) &&
      settings.files.confirmCloseWithUnsavedChanges &&
      !window.confirm("Close this tab with unsaved changes?")
    ) {
      return;
    }
    const index = openTabs.findIndex((tab) => tab.path === path);
    const next = openTabs.filter((tab) => tab.path !== path);
    setOpenTabs(next);
    if (selected?.path !== path) return;
    const fallback = next[Math.min(index, next.length - 1)] ?? null;
    if (fallback) {
      void handleSelect(fallback, { recordHistory: false });
    } else {
      setSelected(null);
      setSession(null);
    }
  }

  function reorderTab(from: string, to: string) {
    if (from === to) return;
    setOpenTabs((tabs) => {
      const fromIndex = tabs.findIndex((tab) => tab.path === from);
      const toIndex = tabs.findIndex((tab) => tab.path === to);
      if (fromIndex < 0 || toIndex < 0) return tabs;
      const next = [...tabs];
      const [moved] = next.splice(fromIndex, 1);
      next.splice(toIndex, 0, moved);
      return next;
    });
  }

  function beginSidebarResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = sidebarWidth;
    const onMove = (move: PointerEvent) => {
      setSidebarWidth(Math.max(210, Math.min(480, startWidth + move.clientX - startX)));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  async function commitTitle() {
    if (!snapshot || !selected) return;
    const nextPath = renamedPath(selected.path, titleDraft);
    if (!titleDraft.trim() || nextPath === selected.path) {
      setEditingTitle(false);
      setTitleDraft(fileTitle(selected.path));
      return;
    }
    if (inBrowser) {
      const nextResource = { ...selected, path: nextPath };
      setSnapshot((current) =>
        current
          ? {
              ...current,
              resources: current.resources.map((resource) =>
                resource.path === selected.path ? nextResource : resource,
              ),
            }
          : current,
      );
      setOpenTabs((tabs) =>
        tabs.map((tab) => (tab.path === selected.path ? nextResource : tab)),
      );
      setSelected(nextResource);
      setEditingTitle(false);
      return;
    }
    setBusy(true);
    try {
      await invoke("rename_resource", {
        root: snapshot.root,
        from: selected.path,
        to: nextPath,
      });
      const resources = await invoke<Resource[]>("list_resources", { root: snapshot.root });
      setSnapshot((current) => (current ? { ...current, resources } : current));
      const nextResource =
        resources.find((resource) => resource.path === nextPath) ?? { ...selected, path: nextPath };
      setOpenTabs((tabs) =>
        tabs.map((tab) => (tab.path === selected.path ? nextResource : tab)),
      );
      navigationController.replacePath(selected.path, nextPath);
      await handleSelect(nextResource, { recordHistory: false });
    } catch (err) {
      setError(String(err));
    } finally {
      setEditingTitle(false);
      setBusy(false);
    }
  }

  function updateWorkspaceSettings(next: {
    capabilities: string[];
    quickNoteDirectory: string;
  }) {
    const current = snapshotRef.current;
    if (!current) return;
    const optimistic = {
      ...current,
      capabilities: next.capabilities,
      defaults: { quickNoteDirectory: next.quickNoteDirectory },
    };
    snapshotRef.current = optimistic;
    setSnapshot(optimistic);
    if (inBrowser) {
      return;
    }
    if (workspaceSettingsTimerRef.current) {
      window.clearTimeout(workspaceSettingsTimerRef.current);
    }
    workspaceSettingsTimerRef.current = window.setTimeout(() => {
      const desired = snapshotRef.current;
      if (!desired) return;
      void updateWorkspaceManifest({
        root: desired.root,
        enabledCapabilities: desired.capabilities,
        quickNoteDirectory: desired.defaults.quickNoteDirectory,
        baseRevision: current.manifestRevision,
      })
        .then((updated) => {
          snapshotRef.current = updated;
          setSnapshot(updated);
          setStatusToast("Workspace settings saved");
        })
        .catch(async (err) => {
          try {
            const reloaded = await invoke<WorkspaceSnapshot>("open_workspace", {
              path: current.root,
            });
            snapshotRef.current = reloaded;
            setSnapshot(reloaded);
          } catch {
            snapshotRef.current = current;
            setSnapshot(current);
          }
          setError(String(err));
        });
    }, 180);
  }

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void saveSidebarWidth(sidebarWidth).catch(() => {});
    }, 250);
    return () => window.clearTimeout(timer);
  }, [sidebarWidth]);

  useEffect(() => {
    if (!snapshot || sessionRestoredRootRef.current === snapshot.root) return;
    sessionRestoredRootRef.current = snapshot.root;
    if (!startup.restoreSession) {
      setActivityArea("home");
      return;
    }
    void loadSession(snapshot.root).then((stored) => {
      if (!stored) {
        setActivityArea("home");
        return;
      }
      const tabs = (stored.tabs ?? [])
        .map((path) => snapshot.resources.find((resource) => resource.path === path))
        .filter((resource): resource is Resource => Boolean(resource));
      setOpenTabs(tabs);
      setInspectorOpen(Boolean(stored.inspector));
      setActivityArea((stored.activity as ActivityArea | null) ?? (tabs.length > 0 ? "files" : "home"));
      const active =
        snapshot.resources.find((resource) => resource.path === stored.active) ?? tabs[0] ?? null;
      if (active) void handleSelect(active, { recordHistory: false });
    }).catch(() => {
      setActivityArea("home");
    });
    // Restoration is intentionally keyed only by the workspace identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snapshot?.root, startup.restoreSession]);

  useEffect(() => {
    if (!snapshot || sessionRestoredRootRef.current !== snapshot.root) return;
    const timer = window.setTimeout(() => {
      void saveSession({
        root: snapshot.root,
        tabs: openTabs.map((tab) => tab.path),
        active: selected?.path ?? null,
        activity: activityArea,
        inspector: inspectorOpen,
      }).catch(() => {});
    }, 250);
    return () => window.clearTimeout(timer);
  }, [snapshot, openTabs, selected?.path, activityArea, inspectorOpen]);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listen<{ root: string; path: string }>("open-resource", (event) => {
      const open = async () => {
        let current = snapshotRef.current;
        if (!current || current.root !== event.payload.root) {
          current = await invoke<WorkspaceSnapshot>("open_workspace", { path: event.payload.root });
          await adoptWorkspace(current);
          sessionRestoredRootRef.current = current.root;
        }
        const resource = current.resources.find((entry) => entry.path === event.payload.path);
        if (resource) await handleSelect(resource);
      };
      void open().catch((err) => setError(String(err)));
    }).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten?.();
    // Listener uses refs / event payload and should only be installed once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (inBrowser) return;
    void register(QUICK_NOTE_SHORTCUT, () => {
      void showQuickNote(snapshotRef.current?.root).catch((err) => setError(String(err)));
    }).catch((err) => {
      console.warn("global Quick Note shortcut unavailable:", err);
    });
    return () => {
      void unregister(QUICK_NOTE_SHORTCUT);
    };
  }, []);

  function openLinkTarget(target: ResourceLinkTarget) {
    if (target.kind === "folder") {
      setActivityArea("files");
      setRevealPath(target.path);
      setStatusToast(`Revealed ${target.path}`);
      return;
    }
    const resource = snapshot?.resources.find((item) => item.path === target.path);
    if (resource) void handleSelect(resource);
  }

  /** Resolve every resource link through the shared Rust catalog. */
  async function handleOpenWiki(target: string) {
    if (!snapshot) return;
    if (inBrowser) {
      const match = wikiTargets.find(
        (candidate) =>
          candidate.canonical.toLowerCase() === target.trim().toLowerCase() ||
          candidate.path.toLowerCase() === target.trim().toLowerCase(),
      );
      if (match) openLinkTarget(match);
      else setError(`No resource found for [[${target}]].`);
      return;
    }
    try {
      const resolution = await resolveResourceLink(
        snapshot.root,
        page?.resource.path ?? null,
        target,
      );
      if (resolution.status === "found") {
        openLinkTarget(resolution.target);
      } else if (resolution.status === "ambiguous") {
        setLinkPicker({ query: resolution.query, candidates: resolution.candidates });
      } else if (
        resolution.suggested_page &&
        window.confirm(`Create ${resolution.suggested_page}?`)
      ) {
        await createAndOpenPage(resolution.suggested_page);
      } else {
        setError(`No resource found for [[${resolution.query}]].`);
      }
    } catch (err) {
      setError(String(err));
    }
  }

  /** A file node's double-click callback: selects it if it's in the workspace. */
  function handleOpenFile(path: string) {
    const resource = snapshot?.resources.find((r) => r.path === path);
    if (resource) void handleSelect(resource);
  }

  const paletteItems = useMemo<PaletteItem[]>(() => {
    const root = snapshot?.root ?? null;
    const actions: PaletteItem[] = [
      { id: "action:new-page", label: "New page", run: handleNewPage },
      { id: "action:new-table", label: "New table…", run: () => void handleNewTable() },
      { id: "action:import-csv", label: "Import CSV…", run: () => void handleImportCsv() },
      { id: "action:quick-note", label: "Quick note", hint: "Cmd+N", run: handleQuickNote },
      { id: "action:new-workspace", label: "New workspace…", run: () => void openNewWorkspaceDialog() },
      { id: "action:open-workspace", label: "Open workspace…", run: () => void handleOpenWorkspace() },
      {
        id: "action:search",
        label: "Search workspace…",
        hint: "Cmd+K",
        run: () => setSearchPaneOpen(true),
      },
      {
        id: "action:theme-follow-system",
        label: "Theme: Follow system",
        hint:
          themeCatalog?.resolved.settings.mode === "auto"
            ? "active"
            : "auto dark/light pair",
        run: () => {
          void (async () => {
            try {
              applyThemeCatalog(await setAppearanceMode("auto", root));
              setStatusToast("Theme follows system");
            } catch (err) {
              setError(String(err));
            }
          })();
        },
      },
    ];

    const themes: ThemeSummaryPayload[] = themeCatalog?.themes ?? [];
    for (const theme of themes) {
      const active = themeCatalog?.resolved.id === theme.id;
      actions.push({
        id: `action:theme-${theme.id}`,
        label: `Theme: ${theme.name}`,
        hint: active
          ? "active"
          : theme.source === "user"
            ? "user"
            : theme.appearance,
        run: () => {
          void (async () => {
            try {
              applyThemeCatalog(await setFixedTheme(theme.id, root));
              setStatusToast(`Theme: ${theme.name}`);
            } catch (err) {
              setError(String(err));
            }
          })();
        },
      });
    }

    if (!inBrowser) {
      actions.push({ id: "action:undo", label: "Undo last change", run: () => void handleUndo() });
    }

    const files: PaletteItem[] = (snapshot?.resources ?? []).map((resource) => ({
      id: `file:${resource.path}`,
      label: resource.path.split("/").pop() ?? resource.path,
      hint: resource.path,
      kind: resource.kind,
      run: () => handleSelect(resource),
    }));

    return [...actions, ...files];
    // Actions and file entries close over `selected`/`snapshot` through the
    // handlers above, which are plain functions recreated every render —
    // depending on the underlying data (not the handlers themselves) keeps
    // this from recomputing on every keystroke without going stale.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snapshot, selected, themeCatalog, applyThemeCatalog]);

  const handleQuickNoteRef = useRef(handleQuickNote);
  handleQuickNoteRef.current = handleQuickNote;
  const handleNewPageRef = useRef(handleNewPage);
  handleNewPageRef.current = handleNewPage;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (matchesKeybinding(event, settings.keybindings.search)) {
        event.preventDefault();
        setPaletteOpen(false);
        setSearchPaneOpen(true);
      } else if (matchesKeybinding(event, settings.keybindings.commandPalette)) {
        event.preventDefault();
        setSearchPaneOpen(false);
        setPaletteOpen(true);
      } else if (matchesKeybinding(event, settings.keybindings.quickNote)) {
        event.preventDefault();
        setPaletteOpen(false);
        handleQuickNoteRef.current();
      } else if (matchesKeybinding(event, settings.keybindings.newPage)) {
        event.preventDefault();
        handleNewPageRef.current();
      } else if (matchesKeybinding(event, settings.keybindings.settings)) {
        event.preventDefault();
        setActivityArea("settings");
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [settings.keybindings]);

  return {
    profile, profileReady, settings, startup, snapshot, snapshotRef, selected, session, error, busy, saveState,
    externalConflict, reloadToken, newWorkspaceOpen, workspacesDir, templates, statusToast, runtimeNotice,
    profileNotices, paletteOpen, searchPaneOpen, themeCatalog, activityArea, sidebarWidth, revealPath, linkPicker,
    openTabs, navigation, inspectorOpen, editingTitle, titleDraft, assetRoot, wikiTargets, pageEditorRef,
    recents, page, currentPageRevisionRef,
    paletteItems, hasCapability, setSettings, setStartup, setSelected, setError, setSnapshot, setSession,
    setSaveState, setExternalConflict, setReloadToken, setNewWorkspaceOpen, setSearchPaneOpen, setPaletteOpen,
    setActivityArea, setInspectorOpen, setDismissedNoticeCodes, setEditingTitle, setTitleDraft, setSidebarWidth,
    setLinkPicker,
    setStatusToast, applyThemeCatalog, rememberWorkspace, clearRecents, resetSettings, handleGetStarted,
    handleOpenWorkspace, openRecent, handleCreateWorkspace, openNewWorkspaceDialog, pickWorkspaceFolder,
    handleNewPage, handleQuickNote, handleNewTable, handleImportCsv, handleUndo, handleSelect,
    handleOpenExternally, handleOpenFile, handleImportEditorAsset, navigateHistory, closeTab, reorderTab,
    beginSidebarResize, commitTitle, updateWorkspaceSettings, handleOpenWiki, openLinkTarget,
    handleKeepIncoming, handleKeepLocal, handleKeepBoth,
  };
}
