import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Resource, WorkspaceChangeEvent, WorkspaceSnapshot } from "./types";
import { KindMark, KIND_LABELS } from "./KindMark";
import { demoCanvas, demoPage, demoSnapshot, demoStartEmpty, inBrowser } from "./demo";
import { CanvasViewer } from "./canvas/CanvasViewer";
import { ConflictEnvelope } from "./editor/ConflictEnvelope";
import {
  PageEditor,
  saveIndicatorText,
  isUnsaved,
  type PageEditorHandle,
  type SaveState,
} from "./editor/PageEditor";
import { createDemoPageIO, createNativePageIO, type PageIO } from "./editor/pageIO";

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
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });
  const [externalConflict, setExternalConflict] = useState<ExternalConflict | null>(null);
  /** Bumped to force a fresh `PageEditor` mount (auto-reload, or a conflict
   * resolution) without tying remounts to `page.revision`, which would
   * otherwise remount on every autosave. */
  const [reloadToken, setReloadToken] = useState(0);

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

  async function handleOpenWorkspace() {
    setError(null);
    const dir = await open({ directory: true, multiple: false, title: "Open Workspace" });
    if (!dir || Array.isArray(dir)) return;

    setBusy(true);
    try {
      const next = await invoke<WorkspaceSnapshot>("open_workspace", { path: dir });
      setSnapshot(next);
      setSelected(null);
      setPage(null);
      setSaveState({ status: "idle" });
      setExternalConflict(null);
      if (!inBrowser) {
        invoke("start_watching", { root: next.root }).catch((err) => {
          console.error("failed to start workspace watcher:", err);
        });
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelect(resource: Resource) {
    setSelected(resource);
    setError(null);
    setPage(null);
    setCanvas(null);
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

    if (resource.kind !== "page" || !snapshot) {
      return;
    }

    setSaveState({ status: "idle" });

    if (inBrowser) {
      setPage({ resource, content: demoPage, revision: "demo:0", io: createDemoPageIO(demoPage) });
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

  /** A file node's double-click callback: selects it if it's in the workspace. */
  function handleOpenFile(path: string) {
    const resource = snapshot?.resources.find((r) => r.path === path);
    if (resource) handleSelect(resource);
  }

  if (!snapshot) {
    return (
      <div className="empty-state" data-tauri-drag-region>
        <BrandMark />
        <h1 className="empty-wordmark">Lattice</h1>
        <p className="empty-copy">
          Open a folder to work in it. Pages, data, notebooks, and canvases stay
          ordinary files on your disk.
        </p>
        <button className="primary-button" onClick={handleOpenWorkspace} disabled={busy}>
          {busy ? "Opening…" : "Open workspace"}
        </button>
        <code className="empty-hint">a workspace is a folder with lattice.yaml</code>
        {error && <p className="error-text">{error}</p>}
      </div>
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
          {snapshot.resources.length === 0 && (
            <div className="resource-list-empty">
              This folder is empty. Files you add appear here.
            </div>
          )}
          {snapshot.resources.map((resource) => (
            <button
              key={resource.path}
              className={
                "resource-item" + (selected?.path === resource.path ? " resource-item-active" : "")
              }
              aria-label={`${KIND_LABELS[resource.kind]}: ${resource.path}`}
              onClick={() => handleSelect(resource)}
            >
              <KindMark kind={resource.kind} />
              <span className="resource-path">{resource.path}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <button className="secondary-button" onClick={handleOpenWorkspace} disabled={busy}>
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
                <p className="placeholder-copy">Select a resource to view it.</p>
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
                <PageEditor
                  key={`${page.resource.path}#${reloadToken}`}
                  ref={pageEditorRef}
                  raw={page.content}
                  revision={page.revision}
                  io={page.io}
                  onSaveStateChange={setSaveState}
                  onRevisionChange={(revision) => {
                    currentPageRevisionRef.current = revision;
                  }}
                />
              </>
            )}
            {selected && selected.kind !== "page" && selected.kind !== "canvas" && !error && (
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
              </div>
            )}
          </div>
        )}
      </main>
    </div>
  );
}
