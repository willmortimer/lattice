import { demoCanvas, demoDataApp, demoPages, demoSearch, demoSnapshot, demoStartEmpty, inBrowser } from "./demo";
import { DataTableView } from "./data/DataTableView";
import type { DataAppSnapshot } from "./data/types";
import { NewWorkspaceDialog } from "./NewWorkspaceDialog";
import { CanvasViewer } from "./canvas/CanvasViewer";
import { BacklinksFooter } from "./BacklinksFooter";
import { CommandPalette, type PaletteItem } from "./CommandPalette";
import { AssetContextProvider } from "./editor/AssetContext";
import { ConflictEnvelope } from "./editor/ConflictEnvelope";
import {
  PageEditor,
  saveIndicatorText,
  isUnsaved,
  type PageEditorHandle,
  type SaveState,
} from "./editor/PageEditor";
import { createDemoPageIO, createNativePageIO, type PageIO } from "./editor/pageIO";
import { listRecentWorkspaces, rememberWorkspace, type RecentWorkspace } from "./lib/recents";
import { fileTimestamp, quickNotePath } from "./lib/timestamp";
import { ResourceTree } from "./ResourceTree";
import { SearchPane } from "./SearchPane";
import { KindMark, KIND_LABELS } from "./KindMark";
import type { Resource, WorkspaceChangeEvent, WorkspaceSnapshot } from "./types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";

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

/** The wordmark node-glyph: the app's own mark, drawn on the lattice grid. */
function BrandMark({ size = 64 }: { size?: number }) {
  return (
    <svg
      className="brand-mark"
      width={size}
      height={size}
      viewBox="0 0 56 56"
      fill="none"
      aria-hidden="true"
    >
      <g stroke="rgba(140, 162, 196, 0.4)" strokeWidth="1">
        <path d="M8 18h40M8 28h40M8 38h40M18 8v40M28 8v40M38 8v40" />
      </g>
      <g stroke="var(--amber)" strokeWidth="1.5">
        <path d="M18 38 28 28l10-10M28 28v10" />
      </g>
      <circle cx="18" cy="38" r="2.5" fill="var(--amber)" />
      <circle cx="38" cy="18" r="2.5" fill="var(--amber)" />
      <circle cx="28" cy="28" r="3.5" fill="var(--amber-bright)" />
    </svg>
  );
}

export default function App() {
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
  const [recents, setRecents] = useState<RecentWorkspace[]>(() => listRecentWorkspaces());
  const [statusToast, setStatusToast] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [searchPaneOpen, setSearchPaneOpen] = useState(false);

  /** The root read-view embeds and search/backlinks commands resolve
   * against — `null` in the in-browser demo shell, which has no real
   * workspace on disk even when `snapshot` holds fixture data. */
  const assetRoot = inBrowser ? null : snapshot?.root ?? null;

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
    setSnapshot(next);
    setSelected(null);
    setPage(null);
    setCanvas(null);
    setDataApp(null);
    setSaveState({ status: "idle" });
    setExternalConflict(null);
    rememberWorkspace(next);
    setRecents(listRecentWorkspaces());
    if (!inBrowser) {
      invoke("start_watching", { root: next.root }).catch((err) => {
        console.error("failed to start workspace watcher:", err);
      });
      // Warm the search index in the background so ⌘K is useful immediately.
      invoke("rebuild_index", { root: next.root }).catch(() => {
        /* index rebuild is best-effort on open */
      });
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
      setRecents(listRecentWorkspaces());
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
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateWorkspace(args: {
    path: string;
    title: string;
    template: string;
  }) {
    setError(null);
    if (inBrowser) {
      setSnapshot({
        ...demoSnapshot,
        root: args.path,
        title: args.title,
        resources: [
          { path: "Home.md", kind: "page" },
          { path: "Inbox", kind: "folder" },
          { path: "Projects", kind: "folder" },
        ],
      });
      setNewWorkspaceOpen(false);
      return;
    }
    setBusy(true);
    try {
      const next = await invoke<WorkspaceSnapshot>("create_workspace", {
        path: args.path,
        title: args.title,
        template: args.template,
      });
      await adoptWorkspace(next);
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

  /** Cmd/Ctrl+N: a new page in `Inbox/`, named by timestamp (docs/07's
   * "Quick-note mode"). */
  function handleQuickNote() {
    const path = quickNotePath();
    setStatusToast(`Capturing to ${path}`);
    window.setTimeout(() => setStatusToast(null), 2200);
    void createAndOpenPage(path);
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

  async function handleSelect(resource: Resource) {
    if (resource.kind === "folder") {
      return;
    }
    setSelected(resource);
    setError(null);
    setPage(null);
    setCanvas(null);
    setDataApp(null);
    setExternalConflict(null);
    setReloadToken(0);
    currentPageRevisionRef.current = null;

    if (resource.kind === "canvas" && snapshot) {
      if (inBrowser) {
        setCanvas({ resource, json: demoCanvas });
        return;
      }

      setBusy(true);
      try {
        const content = await invoke<string>("read_file", {
          root: snapshot.root,
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

    if (resource.kind === "data-app" && snapshot) {
      if (inBrowser) {
        setDataApp({ resource, snapshot: demoDataApp });
        return;
      }

      setBusy(true);
      try {
        const opened = await invoke<DataAppSnapshot>("open_data_app", {
          root: snapshot.root,
          relPath: resource.path,
        });
        setDataApp({ resource, snapshot: opened });
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
      return;
    }

    if (resource.kind !== "page" || !snapshot) {
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
      const io = createNativePageIO(snapshot.root, resource.path);
      const { raw, revision } = await io.load();
      setPage({ resource, content: raw, revision, io });
    } catch (err) {
      setPage(null);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  /** Resolve `[[wiki]]` targets against the open workspace's resources. */
  function handleOpenWiki(target: string) {
    const trimmed = target.trim().replace(/\\/g, "/");
    const candidates = [
      trimmed,
      trimmed.endsWith(".md") ? trimmed : `${trimmed}.md`,
      trimmed.replace(/^\[\[|\]\]$/g, ""),
    ];
    const resources = snapshot?.resources ?? [];
    for (const candidate of candidates) {
      const hit = resources.find(
        (r) =>
          r.kind === "page" &&
          (r.path === candidate ||
            r.path === `${candidate}.md` ||
            r.path.endsWith(`/${candidate}`) ||
            r.path.endsWith(`/${candidate}.md`) ||
            r.path.replace(/\.md$/i, "") === candidate),
      );
      if (hit) {
        void handleSelect(hit);
        return;
      }
    }
    setError(`No page found for [[${target}]].`);
  }

  /** A file node's double-click callback: selects it if it's in the workspace. */
  function handleOpenFile(path: string) {
    const resource = snapshot?.resources.find((r) => r.path === path);
    if (resource) void handleSelect(resource);
  }

  const paletteItems = useMemo<PaletteItem[]>(() => {
    const actions: PaletteItem[] = [
      { id: "action:new-page", label: "New page", run: handleNewPage },
      { id: "action:new-table", label: "New table…", run: () => void handleNewTable() },
      { id: "action:quick-note", label: "Quick note", hint: "Cmd+N", run: handleQuickNote },
      { id: "action:new-workspace", label: "New workspace…", run: () => void openNewWorkspaceDialog() },
      { id: "action:open-workspace", label: "Open workspace…", run: () => void handleOpenWorkspace() },
      {
        id: "action:search",
        label: "Search workspace…",
        hint: "Cmd+K",
        run: () => setSearchPaneOpen(true),
      },
    ];
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
  }, [snapshot, selected]);

  // Cmd/Ctrl+K (search), Cmd/Ctrl+P (palette), Cmd/Ctrl+N (quick note),
  // Cmd/Ctrl+Shift+F (search, legacy).
  const handleQuickNoteRef = useRef(handleQuickNote);
  handleQuickNoteRef.current = handleQuickNote;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey)) return;
      const key = event.key.toLowerCase();

      if (key === "k" || (key === "f" && event.shiftKey)) {
        event.preventDefault();
        setPaletteOpen(false);
        setSearchPaneOpen(true);
      } else if (key === "p") {
        event.preventDefault();
        setSearchPaneOpen(false);
        setPaletteOpen(true);
      } else if (key === "n" && !event.shiftKey) {
        event.preventDefault();
        setPaletteOpen(false);
        handleQuickNoteRef.current();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // Re-open the most recent workspace when launching the native app into
  // the empty state (skip browser demo / ?empty).
  useEffect(() => {
    if (inBrowser || demoStartEmpty || snapshot) return;
    const latest = listRecentWorkspaces()[0];
    if (!latest) return;
    let cancelled = false;
    (async () => {
      try {
        const next = await invoke<WorkspaceSnapshot>("open_workspace", { path: latest.root });
        if (!cancelled) await adoptWorkspace(next);
      } catch {
        // Stale path — leave the empty state; user can pick another recent.
      }
    })();
    return () => {
      cancelled = true;
    };
    // Only on first mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!snapshot) {
    return (
      <>
        <div className="empty-state" data-tauri-drag-region>
          <BrandMark />
          <h1 className="empty-wordmark">Lattice</h1>
          <p className="empty-copy">
            <strong>Create Lattice home</strong> makes{" "}
            <code>~/Lattice</code> (for Settings and Workspaces) and opens your
            first workspace at <code>Workspaces/Personal</code>. Or create a
            workspace in any folder, or open one that already has{" "}
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
          <code className="empty-hint">~/Lattice/Workspaces/Personal · lattice.yaml marks a workspace</code>
          {error && <p className="error-text">{error}</p>}
        </div>
        <NewWorkspaceDialog
          open={newWorkspaceOpen}
          busy={busy}
          workspacesDir={workspacesDir}
          onCancel={() => setNewWorkspaceOpen(false)}
          onCreate={(args) => void handleCreateWorkspace(args)}
        />
      </>
    );
  }

  return (
    <div className="shell">
      <aside className="sidebar">
        <header className="sidebar-head" data-tauri-drag-region>
          <div className="workspace-title" title={snapshot.root}>
            {snapshot.title}
          </div>
          {/* LTR isolate keeps the path reading forward inside the RTL
              ellipsis trick (which trims the head, not the folder name). */}
          <div className="workspace-root">{`⁦${snapshot.root}⁩`}</div>
        </header>
        <nav className="resource-list">
          <ResourceTree
            resources={snapshot.resources}
            selectedPath={selected?.path ?? null}
            onSelect={handleSelect}
          />
        </nav>
        <div className="sidebar-footer">
          <button
            className="secondary-button"
            onClick={() => void openNewWorkspaceDialog()}
            disabled={busy}
          >
            New workspace…
          </button>
          <button className="secondary-button" onClick={() => void handleOpenWorkspace()} disabled={busy}>
            Open workspace…
          </button>
        </div>
      </aside>

      <main className="main-pane">
        <header className="main-head" data-tauri-drag-region>
          {selected && (
            <>
              <KindMark kind={selected.kind} size={13} />
              <span className="main-head-path">{selected.path}</span>
              {selected.kind === "page" && page && (
                <>
                  {isUnsaved(saveState) && (
                    <span
                      className="dirty-dot"
                      title="Unsaved changes"
                      aria-label="Unsaved changes"
                    />
                  )}
                  {saveIndicatorText(saveState) && (
                    <span className={`save-state save-state-${saveState.status}`}>
                      {saveIndicatorText(saveState)}
                    </span>
                  )}
                </>
              )}
            </>
          )}
        </header>
        {selected && selected.kind === "canvas" && canvas ? (
          <div className="canvas-pane">
            <CanvasViewer key={selected.path} json={canvas.json} onOpenFile={handleOpenFile} />
          </div>
        ) : (
          <div className="main-scroll">
            {error && <p className="error-text">{error}</p>}
            {!selected && !error && (
              <div className="placeholder">
                <p className="placeholder-copy">Select a file, or press ⌘K to search.</p>
                <p className="placeholder-sub">⌘N captures a quick note into Inbox/</p>
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
                  <PageEditor
                    key={`${page.resource.path}#${reloadToken}`}
                    ref={pageEditorRef}
                    raw={page.content}
                    revision={page.revision}
                    io={page.io}
                    onSaveStateChange={setSaveState}
                    onOpenWiki={handleOpenWiki}
                    onRevisionChange={(revision) => {
                      currentPageRevisionRef.current = revision;
                    }}
                  />
                </AssetContextProvider>
                <BacklinksFooter
                  root={assetRoot}
                  path={page.resource.path}
                  onOpenFile={handleOpenFile}
                />
              </>
            )}
            {selected && selected.kind === "data-app" && dataApp && snapshot && (
              <DataTableView
                key={selected.path}
                root={snapshot.root}
                relPath={dataApp.resource.path}
                initialSnapshot={dataApp.snapshot}
                demoMutate={inBrowser ? (next) => next : undefined}
              />
            )}
            {selected &&
              selected.kind !== "page" &&
              selected.kind !== "canvas" &&
              selected.kind !== "data-app" &&
              !error && (
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
                  <button
                    className="secondary-button placeholder-open-button"
                    onClick={() => void handleOpenExternally(selected)}
                  >
                    Open externally
                  </button>
                )}
              </div>
            )}
          </div>
        )}
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
      <NewWorkspaceDialog
        open={newWorkspaceOpen}
        busy={busy}
        workspacesDir={workspacesDir}
        onCancel={() => setNewWorkspaceOpen(false)}
        onCreate={(args) => void handleCreateWorkspace(args)}
      />
      {statusToast && <div className="status-toast">{statusToast}</div>}
    </div>
  );
}
