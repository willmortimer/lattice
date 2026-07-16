import { demoCanvas, demoDataApp, demoPages, demoSearch, demoSnapshot, demoStartEmpty, inBrowser } from "./demo";
import type { DataAppSnapshot } from "./data/types";
import { NewWorkspaceDialog } from "./NewWorkspaceDialog";
import { BacklinksFooter } from "./BacklinksFooter";
import { CommandPalette, type PaletteItem } from "./CommandPalette";
import { AssetContextProvider } from "./editor/AssetContext";
import { ConflictEnvelope } from "./editor/ConflictEnvelope";
import type { PageEditorHandle } from "./editor/PageEditor";
import { saveIndicatorText, isUnsaved, type SaveState } from "./editor/saveState";
import { createDemoPageIO, createNativePageIO, type PageIO } from "./editor/pageIO";
import { loadSession, saveSession, saveSidebarWidth } from "./lib/profile";
import {
  demoLinkTargets,
  refreshResourceCatalog,
  resolveResourceLink,
  searchResourceLinks,
  type ResourceLinkTarget,
} from "./lib/resourceLinks";
import {
  listTemplates,
  provisionWorkspace,
  type TemplateDescriptor,
} from "./lib/templates";
import { updateWorkspaceManifest } from "./lib/workspace";
import { installNativeContextMenus, showNativeResourceMenu } from "./lib/nativeMenus";
import { fileTimestamp, quickNotePath } from "./lib/timestamp";
import { ResourceTree } from "./ResourceTree";
import { SearchPane } from "./SearchPane";
import { KindMark, KIND_LABELS } from "./KindMark";
import { QUICK_NOTE_SHORTCUT, showQuickNote } from "./quickNoteWindow";
import { SettingsPage } from "./settings/SettingsPage";
import { matchesKeybinding, useAppSettings } from "./settings/model";
import { BrandMark } from "./shell/BrandMark";
import { HomeDashboard } from "./shell/HomeDashboard";
import { ResourceInspector } from "./shell/ResourceInspector";
import {
  applyResolvedTheme,
  loadThemeCatalog,
  setAppearanceMode,
  setFixedTheme,
  startThemeWatch,
  type ThemeCatalogPayload,
  type ThemeSummaryPayload,
} from "./theme";
import type { Resource, WorkspaceChangeEvent, WorkspaceSnapshot } from "./types";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Button,
  DialogBackdrop,
  DialogPopup,
  DialogPortal,
  DialogRoot,
  DialogTitle,
  IconButton,
  MenuItem,
  MenuPopup,
  MenuPortal,
  MenuPositioner,
  MenuRoot,
  MenuSeparator,
  MenuTrigger,
  TooltipProvider,
} from "@lattice/ui";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUpRight,
  ChevronDown,
  CircleAlert,
  FilePlus2,
  Files,
  Home,
  Menu as MenuIcon,
  MoreHorizontal,
  PanelRight,
  Plus,
  Search,
  Settings,
  Sparkles,
  Table2,
  X,
} from "lucide-react";

interface PageState {
  resource: Resource;
  content: string;
  revision: string | null;
  io: PageIO;
}

/** An external edit landed while this page had unsaved local edits (ADR 0028). */
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

interface CanvasState {
  resource: Resource;
  json: unknown;
}

interface DataAppState {
  resource: Resource;
  snapshot: DataAppSnapshot;
}

type ActivityArea = "home" | "files" | "search" | "quick-note" | "settings";

interface NavigationState {
  paths: string[];
  index: number;
}

function fileTitle(path: string): string {
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

const PageEditor = lazy(() =>
  import("./editor/PageEditor").then((module) => ({ default: module.PageEditor })),
);
const CanvasViewer = lazy(() =>
  import("./canvas/CanvasViewer").then((module) => ({ default: module.CanvasViewer })),
);
const DataTableView = lazy(() =>
  import("./data/DataTableView").then((module) => ({ default: module.DataTableView })),
);

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

export default function App() {
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
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(
    inBrowser && !demoStartEmpty ? demoSnapshot : null,
  );
  const [selected, setSelected] = useState<Resource | null>(null);
  const [page, setPage] = useState<PageState | null>(null);
  const [canvas, setCanvas] = useState<CanvasState | null>(null);
  const [dataApp, setDataApp] = useState<DataAppState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });
  const [externalConflict, setExternalConflict] = useState<ExternalConflict | null>(null);
  /** Bumped to force a fresh `PageEditor` mount (auto-reload, or a conflict
   * resolution) without tying remounts to `page.revision`, which would
   * otherwise remount on every autosave. */
  const [reloadToken, setReloadToken] = useState(0);
  const [newWorkspaceOpen, setNewWorkspaceOpen] = useState(false);
  const [workspacesDir, setWorkspacesDir] = useState<string | null>(null);
  const [templates, setTemplates] = useState<TemplateDescriptor[]>([]);
  const [statusToast, setStatusToast] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [searchPaneOpen, setSearchPaneOpen] = useState(false);
  const [themeCatalog, setThemeCatalog] = useState<ThemeCatalogPayload | null>(null);
  const [activityArea, setActivityArea] = useState<ActivityArea>("files");
  const [sidebarWidth, setSidebarWidth] = useState(272);
  const [revealPath, setRevealPath] = useState<string | null>(null);
  const [linkPicker, setLinkPicker] = useState<{
    query: string;
    candidates: ResourceLinkTarget[];
  } | null>(null);
  const [openTabs, setOpenTabs] = useState<Resource[]>([]);
  const [navigation, setNavigation] = useState<NavigationState>({ paths: [], index: -1 });
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const sessionRestoredRootRef = useRef<string | null>(null);
  const workspaceSettingsTimerRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  useEffect(
    () => installNativeContextMenus(() => settingsRef.current.diagnostics.nativeContextMenus),
    [],
  );

  useEffect(() => {
    document.documentElement.dataset.motion = settings.performance.reducedMotion;
  }, [settings.performance.reducedMotion]);

  useEffect(
    () => () => {
      if (workspaceSettingsTimerRef.current) {
        window.clearTimeout(workspaceSettingsTimerRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    if (profile.sidebarWidth && profile.sidebarWidth >= 210 && profile.sidebarWidth <= 480) {
      setSidebarWidth(profile.sidebarWidth);
    }
  }, [profile.sidebarWidth]);

  useEffect(() => {
    void listTemplates()
      .then(setTemplates)
      .catch((err) => setError(String(err)));
  }, []);

  useEffect(() => {
    const messages = [
      ...profileDiagnostics.map((diagnostic) => `${diagnostic.path}: ${diagnostic.message}`),
      ...(profileSaveError ? [profileSaveError] : []),
    ];
    if (messages.length > 0) setError(messages.join("\n"));
  }, [profileDiagnostics, profileSaveError]);

  /** The root read-view embeds and search/backlinks commands resolve
   * against — `null` in the in-browser demo shell, which has no real
   * workspace on disk even when `snapshot` holds fixture data. */
  const assetRoot = inBrowser ? null : snapshot?.root ?? null;
  const wikiTargets = useMemo(
    () => demoLinkTargets(snapshot?.resources ?? []),
    [snapshot?.resources],
  );
  const hasCapability = useCallback(
    (capability: string) =>
      capability === "pages" || Boolean(snapshot?.capabilities.includes(capability)),
    [snapshot?.capabilities],
  );

  // Refs mirroring state read inside the workspace-changed listener, which
  // subscribes once and must not see stale closures over fast-changing state.
  const pageRef = useRef(page);
  useEffect(() => {
    pageRef.current = page;
  }, [page]);
  const saveStateRef = useRef(saveState);
  useEffect(() => {
    saveStateRef.current = saveState;
  }, [saveState]);
  const snapshotRef = useRef(snapshot);
  useEffect(() => {
    snapshotRef.current = snapshot;
  }, [snapshot]);

  const applyThemeCatalog = useCallback((catalog: ThemeCatalogPayload) => {
    setThemeCatalog(catalog);
    applyResolvedTheme(catalog.resolved);
    const diags = [...catalog.diagnostics, ...catalog.resolved.diagnostics];
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

  /** The revision the open `PageEditor` currently considers its clean base
   * — updated on load/save/reload, not on every keystroke (see
   * `PageEditor`'s `onRevisionChange`). Used to recognize an incoming
   * external-edit event as an echo of a save this window already made. */
  const currentPageRevisionRef = useRef<string | null>(null);
  const pageEditorRef = useRef<PageEditorHandle>(null);

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
      setPage((prev) => (prev ? { ...prev, content: raw, revision } : prev));
      setReloadToken((t) => t + 1);
      setSaveState({ status: "idle" });
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleWorkspaceChanged = useCallback(
    async (event: WorkspaceChangeEvent) => {
      await refreshSidebar();

      const current = pageRef.current;
      if (!current) return;

      if (event.type === "renamed") {
        if (event.from === current.resource.path) {
          setError(`"${event.from}" was renamed to "${event.to}" outside Lattice.`);
          setPage(null);
          setSelected(null);
          setExternalConflict(null);
        }
        return;
      }

      if (event.type === "deleted") {
        if (event.path === current.resource.path) {
          // Defensive second check: macOS atomic replacement can surface a
          // transient remove event. If the page is readable now, reconcile it
          // as a modification instead of ejecting the editor.
          try {
            const disk = await current.io.load();
            if (disk.revision === currentPageRevisionRef.current) return;
            if (isUnsaved(saveStateRef.current)) {
              setExternalConflict({ path: event.path });
            } else {
              setPage((previous) =>
                previous
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
          setPage(null);
          setSelected(null);
          setExternalConflict(null);
        }
        return;
      }

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
    [refreshSidebar, reloadPageFromDisk],
  );

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    listen<WorkspaceChangeEvent>("workspace-changed", (event) => {
      void handleWorkspaceChanged(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [handleWorkspaceChanged]);

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
      setPage((prev) => (prev ? { ...prev, content: localRaw, revision: newRevision } : prev));
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

  async function adoptWorkspace(next: WorkspaceSnapshot) {
    snapshotRef.current = next;
    setSnapshot(next);
    setSelected(null);
    setPage(null);
    setCanvas(null);
    setDataApp(null);
    setSaveState({ status: "idle" });
    setExternalConflict(null);
    setOpenTabs([]);
    setNavigation({ paths: [], index: -1 });
    setActivityArea("home");
    sessionRestoredRootRef.current = null;
    rememberWorkspace(next);
    if (!inBrowser) {
      invoke("start_watching", { root: next.root }).catch((err) => {
        console.error("failed to start workspace watcher:", err);
      });
      // Warm the search index in the background so ⌘K is useful immediately.
      invoke("rebuild_index", { root: next.root }).catch(() => {
        /* index rebuild is best-effort on open */
      });
      void refreshResourceCatalog(next.root);
    }
  }

  async function handleOpenWorkspace() {
    setError(null);
    const dir = await open({ directory: true, multiple: false, title: "Open Workspace" });
    if (!dir || Array.isArray(dir)) return;

    setBusy(true);
    try {
      const next = await invoke<WorkspaceSnapshot>("open_workspace", { path: dir });
      await adoptWorkspace(next);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  /** Creates ~/Lattice (Workspaces + Settings) and opens Workspaces/Personal. */
  async function handleGetStarted() {
    setError(null);
    if (inBrowser) {
      setSnapshot(demoSnapshot);
      rememberWorkspace(demoSnapshot);
      return;
    }
    setBusy(true);
    try {
      const home = await invoke<{
        root: string;
        workspaces: string;
        default_workspace: WorkspaceSnapshot | null;
      }>("ensure_home");
      setWorkspacesDir(home.workspaces);
      if (home.default_workspace) {
        await adoptWorkspace(home.default_workspace);
      } else {
        setError("Lattice home is ready, but no default workspace was found.");
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function openRecent(root: string) {
    setError(null);
    if (inBrowser) {
      setSnapshot(demoSnapshot);
      return;
    }
    setBusy(true);
    try {
      const next = await invoke<WorkspaceSnapshot>("open_workspace", { path: root });
      await adoptWorkspace(next);
    } catch (err) {
      removeRecent(root);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateWorkspace(args: {
    path: string;
    title: string;
    template: string;
    setDefault: boolean;
    initializeExisting: boolean;
  }) {
    setError(null);
    if (inBrowser) {
      const outcome = await provisionWorkspace(args);
      await adoptWorkspace(outcome.workspace);
      refreshProfile();
      setNewWorkspaceOpen(false);
      return;
    }
    setBusy(true);
    try {
      const outcome = await provisionWorkspace(args);
      await adoptWorkspace(outcome.workspace);
      refreshProfile();
      if (outcome.diagnostics.length > 0) {
        setStatusToast(outcome.diagnostics.map((item) => item.message).join(" "));
      } else {
        setStatusToast(`Created ${outcome.workspace.title}`);
      }
      setNewWorkspaceOpen(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function openNewWorkspaceDialog() {
    setError(null);
    if (!inBrowser && !workspacesDir) {
      try {
        const home = await invoke<{ workspaces: string }>("ensure_home");
        setWorkspacesDir(home.workspaces);
      } catch (err) {
        setError(String(err));
        return;
      }
    }
    setNewWorkspaceOpen(true);
  }

  async function pickWorkspaceFolder() {
    const path = await open({
      directory: true,
      multiple: false,
      title: "Choose workspace destination",
    });
    return typeof path === "string" ? path : null;
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
      setCanvas(null);
      setDataApp(null);
      setExternalConflict(null);
      setSaveState({ status: "idle" });
      setReloadToken((t) => t + 1);
      currentPageRevisionRef.current = "demo:0";
      setPage({ resource, content: "", revision: "demo:0", io: createDemoPageIO("") });
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
      setPage(null);
      setCanvas(null);
      setDataApp({ resource, snapshot: created });
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
      setPage(null);
      setCanvas(null);
      setExternalConflict(null);
      setError(null);
      setDataApp({
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
      setPage(null);
      setCanvas(null);
      setDataApp({ resource, snapshot: created });
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

  async function handleSelect(
    resource: Resource,
    options: { recordHistory?: boolean } = {},
  ) {
    const workspace = snapshotRef.current ?? snapshot;
    if (resource.kind === "folder") {
      return;
    }
    setActivityArea("files");
    setOpenTabs((tabs) => {
      if (tabs.some((tab) => tab.path === resource.path)) return tabs;
      return [...tabs, resource].slice(-settings.performance.maxOpenTabs);
    });
    if (options.recordHistory !== false) {
      setNavigation((current) => {
        if (current.paths[current.index] === resource.path) return current;
        const paths = [...current.paths.slice(0, current.index + 1), resource.path].slice(-60);
        return { paths, index: paths.length - 1 };
      });
    }
    setSelected(resource);
    setTitleDraft(fileTitle(resource.path));
    setEditingTitle(false);
    setError(null);
    setPage(null);
    setCanvas(null);
    setDataApp(null);
    setExternalConflict(null);
    setReloadToken(0);
    currentPageRevisionRef.current = null;

    if (resource.kind === "canvas" && workspace) {
      if (!hasCapability("canvas")) {
        setError("Canvas is not enabled for this workspace.");
        return;
      }
      if (inBrowser) {
        setCanvas({ resource, json: demoCanvas });
        return;
      }

      setBusy(true);
      try {
        const content = await invoke<string>("read_file", {
          root: workspace.root,
          relPath: resource.path,
        });
        setCanvas({ resource, json: JSON.parse(content) });
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
      return;
    }

    if (resource.kind === "data-app" && workspace) {
      if (!hasCapability("sqlite")) {
        setError("Data apps are not enabled for this workspace.");
        return;
      }
      if (inBrowser) {
        setDataApp({ resource, snapshot: demoDataApp });
        return;
      }

      setBusy(true);
      try {
        const opened = await invoke<DataAppSnapshot>("open_data_app", {
          root: workspace.root,
          relPath: resource.path,
          viewName: null,
        });
        setDataApp({ resource, snapshot: opened });
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
      return;
    }

    if (resource.kind !== "page" || !workspace) {
      return;
    }

    setSaveState({ status: "idle" });

    if (inBrowser) {
      const content = demoPages[resource.path] ?? `# ${resource.path}\n`;
      setPage({
        resource,
        content,
        revision: "demo:0",
        io: createDemoPageIO(content),
      });
      return;
    }

    setBusy(true);
    try {
      const io = createNativePageIO(workspace.root, resource.path);
      const { raw, revision } = await io.load();
      setPage({ resource, content: raw, revision, io });
    } catch (err) {
      setPage(null);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  function navigateHistory(delta: -1 | 1) {
    const nextIndex = navigation.index + delta;
    if (nextIndex < 0 || nextIndex >= navigation.paths.length) return;
    const path = navigation.paths[nextIndex];
    const resource = snapshot?.resources.find((entry) => entry.path === path);
    if (!resource) return;
    setNavigation((current) => ({ ...current, index: nextIndex }));
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
      setPage(null);
      setCanvas(null);
      setDataApp(null);
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
      setNavigation((current) => ({
        ...current,
        paths: current.paths.map((path) => (path === selected.path ? nextPath : path)),
      }));
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

  // Resume the last valid workspace, then fall back to the configured
  // effective default. Invalid paths remain visible in profile diagnostics.
  useEffect(() => {
    if (inBrowser || demoStartEmpty || snapshot || !profileReady) return;
    const candidates = [
      ...(startup.reopenLastWorkspace ? recents.map((recent) => recent.root) : []),
      profile.effectiveDefaultWorkspace,
    ].filter((path, index, all): path is string => Boolean(path) && all.indexOf(path) === index);
    if (candidates.length === 0) return;
    let cancelled = false;
    (async () => {
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
    return () => {
      cancelled = true;
    };
    // Only on first mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    profileReady,
    profile.effectiveDefaultWorkspace,
    recents,
    snapshot,
    startup.reopenLastWorkspace,
    removeRecent,
  ]);

  if (!snapshot) {
    return (
      <>
        <div className="native-titlebar" data-tauri-drag-region />
        <div className="empty-state">
          <BrandMark />
          <h1 className="empty-wordmark">Lattice</h1>
          <p className="empty-copy">
            <strong>Create Lattice home</strong> makes{" "}
            <code>~/Lattice</code> (for Settings and Workspaces) and opens your
            default Personal workspace. Or choose a starting point from the
            workspace gallery, create in any folder, or open one that already has{" "}
            <code>lattice.yaml</code>.
          </p>
          <div className="empty-actions">
            <button className="primary-button" onClick={() => void handleGetStarted()} disabled={busy}>
              {busy ? "Setting up…" : "Create Lattice home"}
            </button>
            <button
              className="secondary-button"
              onClick={() => void openNewWorkspaceDialog()}
              disabled={busy}
            >
              New workspace in a folder…
            </button>
            <button className="secondary-button" onClick={() => void handleOpenWorkspace()} disabled={busy}>
              Open existing workspace…
            </button>
          </div>
          {recents.length > 0 && (
            <div className="recent-workspaces">
              <div className="recent-heading">Recent</div>
              {recents.slice(0, 5).map((r) => (
                <button
                  key={r.root}
                  type="button"
                  className="recent-item"
                  onClick={() => void openRecent(r.root)}
                  disabled={busy}
                  title={r.root}
                >
                  <span className="recent-title">{r.title}</span>
                  <code className="recent-path">{r.root}</code>
                </button>
              ))}
            </div>
          )}
          <code className="empty-hint">Your default workspace can be changed when creating another workspace.</code>
          {error && <p className="error-text">{error}</p>}
        </div>
        <NewWorkspaceDialog
          open={newWorkspaceOpen}
          busy={busy}
          templates={templates}
          workspacesDir={workspacesDir}
          hasValidDefault={profile.hasValidConfiguredDefault}
          onCancel={() => setNewWorkspaceOpen(false)}
          onPickFolder={pickWorkspaceFolder}
          onCreate={(args) => void handleCreateWorkspace(args)}
        />
      </>
    );
  }

  return (
    <TooltipProvider>
      <div className="shell">
        <div className="native-titlebar" data-tauri-drag-region />
        <aside className="activity-rail" aria-label="Workspace areas">
          <div className="activity-brand">
            <BrandMark size={28} />
          </div>
          <nav>
            {[
              { id: "home" as const, label: "Home", icon: Home },
              { id: "files" as const, label: "Files", icon: Files },
              { id: "search" as const, label: "Search", icon: Search },
              { id: "quick-note" as const, label: "Quick Capture", icon: Sparkles },
            ].map(({ id, label, icon: Icon }) => (
              <IconButton
                key={id}
                label={label}
                className={activityArea === id ? "activity-button-active" : ""}
                onClick={() => {
                  if (id === "search") {
                    setActivityArea("search");
                    setSearchPaneOpen(true);
                  } else if (id === "quick-note") {
                    setActivityArea("quick-note");
                    handleQuickNote();
                  } else {
                    setActivityArea(id);
                  }
                }}
              >
                <Icon size={17} />
              </IconButton>
            ))}
          </nav>
          <div className="activity-spacer" />
          <IconButton
            label="Settings"
            className={activityArea === "settings" ? "activity-button-active" : ""}
            onClick={() => setActivityArea("settings")}
          >
            <Settings size={17} />
          </IconButton>
        </aside>

        <aside className="sidebar" style={{ width: sidebarWidth }}>
          <header className="sidebar-head">
            <div className="workspace-title-row">
              <div className="workspace-title" title={snapshot.root}>
                {snapshot.title}
              </div>
              <IconButton label="Workspace menu" onClick={() => setPaletteOpen(true)}>
                <MoreHorizontal size={15} />
              </IconButton>
            </div>
            <div className="workspace-root">{`⁦${snapshot.root}⁩`}</div>
          </header>
          <div className="sidebar-toolbar">
            <Button
              variant="ghost"
              size="sm"
              className="sidebar-search"
              onClick={() => setSearchPaneOpen(true)}
            >
              <Search size={14} />
              Search
              <kbd>{settings.keybindings.search}</kbd>
            </Button>
            <MenuRoot>
              <MenuTrigger
                render={
                  <IconButton label="Create resource">
                    <Plus size={15} />
                  </IconButton>
                }
              />
              <MenuPortal>
                <MenuPositioner sideOffset={6} align="end">
                  <MenuPopup className="ltui-menu">
                    <MenuItem className="ltui-menu-item" onClick={handleNewPage}>
                      <FilePlus2 size={14} />
                      New page
                    </MenuItem>
                    {hasCapability("sqlite") && (
                      <MenuItem className="ltui-menu-item" onClick={() => void handleNewTable()}>
                        <Table2 size={14} />
                        New table
                      </MenuItem>
                    )}
                    <MenuSeparator className="ltui-menu-separator" />
                    {hasCapability("sqlite") && (
                      <MenuItem className="ltui-menu-item" onClick={() => void handleImportCsv()}>
                        <ArrowUpRight size={14} />
                        Import CSV
                      </MenuItem>
                    )}
                  </MenuPopup>
                </MenuPositioner>
              </MenuPortal>
            </MenuRoot>
          </div>
          <nav className="resource-list">
            <ResourceTree
              resources={snapshot.resources}
              selectedPath={selected?.path ?? null}
              onSelect={handleSelect}
              onContextMenu={(resource) =>
                void showNativeResourceMenu({
                  open: () => void handleSelect(resource),
                  inspect: () => {
                    void handleSelect(resource);
                    setInspectorOpen(true);
                  },
                  openExternally: !inBrowser
                    ? () => void handleOpenExternally(resource)
                    : undefined,
                })
              }
              revealPath={revealPath}
            />
          </nav>
          <div className="sidebar-footer">
            <Button variant="ghost" size="sm" onClick={() => void openNewWorkspaceDialog()}>
              New workspace…
            </Button>
            <Button variant="ghost" size="sm" onClick={() => void handleOpenWorkspace()}>
              Open workspace…
            </Button>
          </div>
          <div
            className="sidebar-resize"
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize resource sidebar"
            onPointerDown={beginSidebarResize}
          />
        </aside>

        <main className="main-pane">
          <header className="main-head">
            <div className="nav-controls">
              <IconButton
                label="Back"
                disabled={navigation.index <= 0}
                onClick={() => navigateHistory(-1)}
              >
                <ArrowLeft size={15} />
              </IconButton>
              <IconButton
                label="Forward"
                disabled={navigation.index >= navigation.paths.length - 1}
                onClick={() => navigateHistory(1)}
              >
                <ArrowRight size={15} />
              </IconButton>
            </div>
            <div className="breadcrumbs">
              <button type="button" onClick={() => setActivityArea("home")}>
                {snapshot.title}
              </button>
              {selected?.path.split("/").slice(0, -1).map((part, index) => (
                <span key={`${part}:${index}`}>
                  <ChevronDown size={11} />
                  {part}
                </span>
              ))}
              {selected && (
                <>
                  <ChevronDown size={11} />
                  <KindMark kind={selected.kind} size={13} />
                  {editingTitle ? (
                    <input
                      className="title-input"
                      value={titleDraft}
                      autoFocus
                      onChange={(event) => setTitleDraft(event.target.value)}
                      onBlur={() => void commitTitle()}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") void commitTitle();
                        if (event.key === "Escape") {
                          setEditingTitle(false);
                          setTitleDraft(fileTitle(selected.path));
                        }
                      }}
                    />
                  ) : (
                    <button
                      type="button"
                      className="resource-title-button"
                      onDoubleClick={() => setEditingTitle(true)}
                      title="Double-click to rename"
                    >
                      {fileTitle(selected.path)}
                    </button>
                  )}
                </>
              )}
            </div>
            <div className="header-actions">
              {selected?.kind === "page" && page && (
                <span className={`save-state save-state-${saveState.status}`}>
                  {externalConflict ? "Conflict" : saveIndicatorText(saveState) || "Saved"}
                </span>
              )}
              {selected && !inBrowser && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => void handleOpenExternally(selected)}
                >
                  <ArrowUpRight size={13} />
                  Open
                </Button>
              )}
              <IconButton
                label={inspectorOpen ? "Hide inspector" : "Show inspector"}
                className={inspectorOpen ? "header-button-active" : ""}
                onClick={() => setInspectorOpen((open) => !open)}
              >
                <PanelRight size={16} />
              </IconButton>
              <IconButton label="Command palette" onClick={() => setPaletteOpen(true)}>
                <MenuIcon size={16} />
              </IconButton>
            </div>
          </header>

          {openTabs.length > 0 && (
            <div className="tab-strip" role="tablist" aria-label="Open resources">
              {openTabs.map((tab) => (
                <button
                  type="button"
                  role="tab"
                  aria-selected={selected?.path === tab.path && activityArea === "files"}
                  draggable
                  className={selected?.path === tab.path ? "resource-tab resource-tab-active" : "resource-tab"}
                  key={tab.path}
                  onClick={() => void handleSelect(tab)}
                  onDragStart={(event) => event.dataTransfer.setData("text/lattice-tab", tab.path)}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) =>
                    reorderTab(event.dataTransfer.getData("text/lattice-tab"), tab.path)
                  }
                >
                  <KindMark kind={tab.kind} size={12} />
                  <span>{fileTitle(tab.path)}</span>
                  {tab.path === selected?.path && isUnsaved(saveState) && <i />}
                  <span
                    className="tab-close"
                    role="button"
                    tabIndex={0}
                    aria-label={`Close ${fileTitle(tab.path)}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      closeTab(tab.path);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") closeTab(tab.path);
                    }}
                  >
                    <X size={12} />
                  </span>
                </button>
              ))}
            </div>
          )}

          <div className="workspace-content">
            <section className="content-pane">
              {activityArea === "home" && (
                <HomeDashboard
                  title={snapshot.title}
                  resourceCount={snapshot.resources.length}
                  onNewPage={handleNewPage}
                  onQuickCapture={handleQuickNote}
                  onFiles={() => setActivityArea("files")}
                  onSearch={() => setSearchPaneOpen(true)}
                  onInspect={() => setInspectorOpen(true)}
                />
              )}

              {activityArea === "settings" && (
                <SettingsPage
                  settings={settings}
                  startup={startup}
                  workspace={snapshot}
                  themeCatalog={themeCatalog}
                  onChange={setSettings}
                  onStartupChange={setStartup}
                  onWorkspaceChange={(next) => void updateWorkspaceSettings(next)}
                  onClearRecents={clearRecents}
                  onReset={resetSettings}
                  onThemeChange={(themeId) =>
                    void setFixedTheme(themeId, snapshot.root)
                      .then(applyThemeCatalog)
                      .catch((err) => setError(String(err)))
                  }
                  onFollowSystem={() =>
                    void setAppearanceMode("auto", snapshot.root)
                      .then(applyThemeCatalog)
                      .catch((err) => setError(String(err)))
                  }
                />
              )}

              {activityArea !== "home" &&
                activityArea !== "settings" &&
                selected &&
                selected.kind === "canvas" &&
                canvas && (
                  <div className="canvas-pane">
                    <Suspense fallback={<div className="surface-loading">Loading canvas…</div>}>
                      <CanvasViewer
                        key={selected.path}
                        json={canvas.json}
                        onOpenFile={handleOpenFile}
                      />
                    </Suspense>
                  </div>
                )}

              {activityArea !== "home" &&
                activityArea !== "settings" &&
                !(selected?.kind === "canvas" && canvas) && (
                  <div className="main-scroll">
                    {!selected && (
                      <div className="placeholder">
                        <p className="placeholder-copy">Select a resource from Files.</p>
                        <p className="placeholder-sub">
                          ⌘N opens Quick Note · {QUICK_NOTE_SHORTCUT} works globally
                        </p>
                      </div>
                    )}
                    {selected && selected.kind === "page" && page && (
                      <>
                        {externalConflict && (
                          <ConflictEnvelope
                            message={`"${externalConflict.path}" changed on disk while you had unsaved edits.`}
                            actions={[
                              { label: "Keep incoming", onClick: () => void handleKeepIncoming() },
                              { label: "Keep local", onClick: () => void handleKeepLocal() },
                              { label: "Keep both", onClick: () => void handleKeepBoth(), variant: "primary" },
                            ]}
                          />
                        )}
                        <AssetContextProvider value={{ root: assetRoot, pagePath: page.resource.path }}>
                          <Suspense fallback={<div className="surface-loading">Loading editor…</div>}>
                            <PageEditor
                              key={`${page.resource.path}#${reloadToken}`}
                              ref={pageEditorRef}
                              raw={page.content}
                              revision={page.revision}
                              io={page.io}
                              onSaveStateChange={setSaveState}
                              onOpenWiki={(target) => {
                                handleOpenWiki(target);
                                if (settings.editor.linkClickBehavior === "inspect") {
                                  setInspectorOpen(true);
                                }
                              }}
                              onCreateTable={handleNewTable}
                              wikiTargets={wikiTargets}
                              onSearchWiki={
                                !inBrowser && snapshot
                                  ? (query) => searchResourceLinks(snapshot.root, query, 20)
                                  : undefined
                              }
                              onImportAsset={inBrowser ? undefined : handleImportEditorAsset}
                              autosaveDelayMs={settings.editor.autosaveDelayMs}
                              spellcheck={settings.editor.spellcheck}
                              slashCommands={settings.editor.slashCommands}
                              showFrontmatter={settings.editor.showFrontmatter}
                              onRevisionChange={(revision) => {
                                currentPageRevisionRef.current = revision;
                              }}
                            />
                          </Suspense>
                        </AssetContextProvider>
                        <BacklinksFooter
                          root={assetRoot}
                          path={page.resource.path}
                          onOpenFile={handleOpenFile}
                        />
                      </>
                    )}
                    {selected && selected.kind === "data-app" && dataApp && (
                      <Suspense fallback={<div className="surface-loading">Loading table…</div>}>
                        <DataTableView
                          key={selected.path}
                          root={snapshot.root}
                          relPath={dataApp.resource.path}
                          initialSnapshot={dataApp.snapshot}
                          demoMutate={inBrowser ? (next) => next : undefined}
                          preferences={settings.data}
                          showRendererStats={settings.diagnostics.showRendererStats}
                        />
                      </Suspense>
                    )}
                    {selected &&
                      selected.kind !== "page" &&
                      selected.kind !== "canvas" &&
                      selected.kind !== "data-app" && (
                        <div className="placeholder">
                          <span className="placeholder-mark">
                            <KindMark kind={selected.kind} size={36} />
                          </span>
                          <p className="placeholder-copy">
                            No {KIND_LABELS[selected.kind].toLowerCase()} viewer yet.
                          </p>
                          <p className="placeholder-sub">
                            The file stays yours — open <code>{selected.path}</code> in any tool.
                          </p>
                          {!inBrowser && (
                            <Button
                              variant="secondary"
                              onClick={() => void handleOpenExternally(selected)}
                            >
                              Open externally
                            </Button>
                          )}
                        </div>
                      )}
                  </div>
                )}

              {error && (
                <div className="bottom-panel" role="alert">
                  <CircleAlert size={15} />
                  <div>
                    <strong>Problem</strong>
                    <span>{error}</span>
                  </div>
                  <IconButton label="Dismiss problem" onClick={() => setError(null)}>
                    <X size={14} />
                  </IconButton>
                </div>
              )}
              {!error && busy && (
                <div className="bottom-panel bottom-panel-job" aria-live="polite">
                  <span className="job-spinner" />
                  <div>
                    <strong>Working</strong>
                    <span>Loading or applying a bounded workspace operation…</span>
                  </div>
                </div>
              )}
            </section>

            {inspectorOpen && (
              <ResourceInspector
                root={assetRoot}
                resource={selected}
                pageContent={page?.content ?? null}
                dataSnapshot={dataApp?.snapshot ?? null}
                error={error}
                onClose={() => setInspectorOpen(false)}
                onOpenFile={handleOpenFile}
              />
            )}
          </div>
        </main>

      {paletteOpen && (
        <CommandPalette items={paletteItems} onClose={() => setPaletteOpen(false)} />
      )}
      {searchPaneOpen && (
        <SearchPane
          root={assetRoot}
          demoSearch={inBrowser ? demoSearch : () => []}
          onOpenFile={(path) => {
            setSearchPaneOpen(false);
            handleOpenFile(path);
          }}
          onClose={() => setSearchPaneOpen(false)}
        />
      )}
      {linkPicker && (
        <DialogRoot open onOpenChange={(open) => !open && setLinkPicker(null)}>
          <DialogPortal>
            <DialogBackdrop className="modal-backdrop" />
            <DialogPopup className="modal-panel link-picker-panel">
            <DialogTitle id="link-picker-title">Choose “{linkPicker.query}”</DialogTitle>
            <p className="modal-copy">More than one resource matches this link.</p>
            <div className="link-picker-list">
              {linkPicker.candidates.map((candidate) => (
                <button
                  type="button"
                  key={candidate.path}
                  onClick={() => {
                    openLinkTarget(candidate);
                    setLinkPicker(null);
                  }}
                >
                  <KindMark kind={candidate.kind} size={14} />
                  <span>
                    <strong>{candidate.display}</strong>
                    <small>{candidate.path}</small>
                  </span>
                </button>
              ))}
            </div>
            <div className="modal-actions">
              <Button onClick={() => setLinkPicker(null)}>Cancel</Button>
            </div>
            </DialogPopup>
          </DialogPortal>
        </DialogRoot>
      )}
      <NewWorkspaceDialog
        open={newWorkspaceOpen}
        busy={busy}
        templates={templates}
        workspacesDir={workspacesDir}
        hasValidDefault={profile.hasValidConfiguredDefault}
        onCancel={() => setNewWorkspaceOpen(false)}
        onPickFolder={pickWorkspaceFolder}
        onCreate={(args) => void handleCreateWorkspace(args)}
      />
      {statusToast && <div className="status-toast">{statusToast}</div>}
      </div>
    </TooltipProvider>
  );
}
