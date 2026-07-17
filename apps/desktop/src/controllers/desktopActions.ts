import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { resolveResourceLink, type ResourceLinkTarget } from "../lib/resourceLinks";
import { createPage } from "../lib/pages";
import { updateWorkspaceManifest } from "../lib/workspace";
import { fileTimestamp, quickNotePath } from "../lib/timestamp";
import { showQuickNote } from "../quickNoteWindow";
import { createDemoPageIO } from "../editor/pageIO";
import type { DataAppSnapshot } from "../data/types";
import type { OpenResourceSession } from "../resourceSession";
import type { Resource, WorkspaceSnapshot } from "../types";
import { inBrowser } from "../demo";

function dirnameOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash >= 0 ? path.slice(0, slash) : "";
}

function tableNameFromLabel(label: string): string {
  let name = label.trim().replace(/\.data$/i, "").toLowerCase()
    .replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
  if (!name || /^\d/.test(name)) name = `t_${name || "table"}`;
  return name;
}

function dataPackagePath(label: string): string {
  return `${label.trim().replace(/\.data$/i, "")}.data`;
}

type PageSession = Extract<OpenResourceSession, { kind: "page" }>;

export interface DesktopActionsOptions {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  setSnapshot: Dispatch<SetStateAction<WorkspaceSnapshot | null>>;
  selected: Resource | null;
  pageRef: MutableRefObject<PageSession | null>;
  wikiTargets: ResourceLinkTarget[];
  setError: (message: string | null) => void;
  setBusy: (busy: boolean) => void;
  setStatusToast: (message: string | null) => void;
  setSaveStateIdle: () => void;
  setActivityArea: (area: "files") => void;
  setRevealPath: (path: string | null) => void;
  setLinkPicker: (picker: { query: string; candidates: ResourceLinkTarget[] } | null) => void;
  refreshResources: () => Promise<void>;
  handleSelect: (resource: Resource, options?: { recordHistory?: boolean }) => Promise<void>;
  openCreatedResource: (resource: Resource, session: OpenResourceSession) => void;
}

export function useDesktopActionsController(options: DesktopActionsOptions) {
  const {
    snapshot, snapshotRef, setSnapshot, selected, pageRef, wikiTargets, setError, setBusy,
    setStatusToast, setSaveStateIdle, setActivityArea, setRevealPath, setLinkPicker,
    refreshResources, handleSelect, openCreatedResource,
  } = options;
  const workspaceSettingsTimerRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  useEffect(() => () => {
    if (workspaceSettingsTimerRef.current) window.clearTimeout(workspaceSettingsTimerRef.current);
  }, []);

  const createAndOpenPage = useCallback(async (
    relPath: string,
    options?: { templatePath?: string | null; title?: string | null; content?: string },
  ) => {
    const resource: Resource = { path: relPath, kind: "page" };
    if (inBrowser) {
      setSnapshot((prev) => prev ? { ...prev, resources: [...prev.resources, resource] } : prev);
      setError(null);
      setSaveStateIdle();
      openCreatedResource(resource, {
        kind: "page",
        resource,
        content: options?.content ?? "",
        revision: "demo:0",
        io: createDemoPageIO(options?.content ?? ""),
      });
      return;
    }
    if (!snapshot) return;
    setBusy(true);
    try {
      await createPage({
        root: snapshot.root,
        relPath,
        content: options?.content ?? "",
        templatePath: options?.templatePath,
        title: options?.title,
      });
      await refreshResources();
      await handleSelect(resource);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [handleSelect, openCreatedResource, refreshResources, setBusy, setError, setSaveStateIdle, setSnapshot, snapshot]);

  const handleQuickNote = useCallback(() => {
    if (inBrowser) {
      const path = quickNotePath(new Date(), snapshot?.defaults.quickNoteDirectory ?? "Inbox");
      setStatusToast(`Browser demo capture: ${path}`);
      window.setTimeout(() => setStatusToast(null), 2200);
      void createAndOpenPage(path);
      return;
    }
    void showQuickNote(snapshot?.root).catch((error) => setError(String(error)));
  }, [createAndOpenPage, setError, setStatusToast, snapshot]);

  const handleNewPage = useCallback(() => {
    const dir = selected ? dirnameOf(selected.path) : "";
    const name = `Untitled ${fileTimestamp()}.md`;
    void createAndOpenPage(dir ? `${dir}/${name}` : name);
  }, [createAndOpenPage, selected]);

  const handleUndo = useCallback(async () => {
    if (!snapshot) return;
    try {
      await invoke<string | null>("undo_last", { root: snapshot.root });
      await refreshResources();
    } catch (error) {
      setError(String(error));
    }
  }, [refreshResources, setError, snapshot]);

  const handleImportCsv = useCallback(async () => {
    if (inBrowser) {
      setError("CSV import is not available in the browser demo.");
      return;
    }
    if (!snapshot) return;
    const selectedFile = await open({ multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!selectedFile || typeof selectedFile !== "string") return;
    const name = window.prompt("Package name", "Imported")?.trim();
    if (!name) return;
    setBusy(true);
    try {
      const [relPath, created] = await invoke<[string, DataAppSnapshot]>("import_csv_table", {
        root: snapshot.root, csvPath: selectedFile, packageName: name,
        title: name.replace(/\.data$/i, ""), tableName: tableNameFromLabel(name),
      });
      await refreshResources();
      const resource: Resource = { path: relPath, kind: "data-app" };
      openCreatedResource(resource, { kind: "data-app", resource, snapshot: created });
      setError(null);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [openCreatedResource, refreshResources, setBusy, setError, snapshot]);

  const handleNewTable = useCallback(async () => {
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
      setSnapshot((prev) => prev ? { ...prev, resources: [...prev.resources, resource] } : prev);
      openCreatedResource(resource, {
        kind: "data-app", resource,
        snapshot: { title, default_table: tableName, package_revision: "demo:0",
          columns: [{ name: "id", field_type: "text", sqlite_type: "TEXT" }], rows: [],
          available_views: ["All"], active_view: "All", filters: [] },
      });
      return;
    }
    if (!snapshot) return;
    setBusy(true);
    try {
      const created = await invoke<DataAppSnapshot>("create_table_package", {
        root: snapshot.root, relPath, title, tableName,
      });
      await refreshResources();
      openCreatedResource(resource, { kind: "data-app", resource, snapshot: created });
      setError(null);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [openCreatedResource, refreshResources, setBusy, setError, setSnapshot, snapshot]);

  const handleImportEditorAsset = useCallback(async (file: File): Promise<string> => {
    const workspace = snapshotRef.current;
    const currentPage = pageRef.current;
    if (inBrowser || !workspace || !currentPage) throw new Error("File import requires the native desktop workspace.");
    const content = new Uint8Array(await file.arrayBuffer());
    const relativePath = await invoke<string>("create_asset", content, {
      headers: {
        "x-lattice-root": encodeURIComponent(workspace.root),
        "x-lattice-page-path": encodeURIComponent(currentPage.resource.path),
        "x-lattice-file-name": encodeURIComponent(file.name),
      },
    });
    await refreshResources();
    return relativePath;
  }, [pageRef, refreshResources, snapshotRef]);

  const handleOpenExternally = useCallback(async (resource: Resource) => {
    if (!snapshot) return;
    try {
      await openPath(`${snapshot.root}/${resource.path}`);
    } catch (error) {
      setError(String(error));
    }
  }, [setError, snapshot]);

  const openLinkTarget = useCallback((target: ResourceLinkTarget) => {
    if (target.kind === "folder") {
      setActivityArea("files");
      setRevealPath(target.path);
      setStatusToast(`Revealed ${target.path}`);
      return;
    }
    const resource = snapshot?.resources.find((item) => item.path === target.path);
    if (resource) void handleSelect(resource);
  }, [handleSelect, setActivityArea, setRevealPath, setStatusToast, snapshot]);

  const handleOpenWiki = useCallback(async (target: string) => {
    if (!snapshot) return;
    if (inBrowser) {
      const match = wikiTargets.find((candidate) =>
        candidate.canonical.toLowerCase() === target.trim().toLowerCase() ||
        candidate.path.toLowerCase() === target.trim().toLowerCase());
      if (match) openLinkTarget(match);
      else setError(`No resource found for [[${target}]].`);
      return;
    }
    try {
      const resolution = await resolveResourceLink(snapshot.root, pageRef.current?.resource.path ?? null, target);
      if (resolution.status === "found") openLinkTarget(resolution.target);
      else if (resolution.status === "ambiguous") setLinkPicker({ query: resolution.query, candidates: resolution.candidates });
      else if (resolution.suggested_page && window.confirm(`Create ${resolution.suggested_page}?`)) await createAndOpenPage(resolution.suggested_page);
      else setError(`No resource found for [[${resolution.query}]].`);
    } catch (error) {
      setError(String(error));
    }
  }, [createAndOpenPage, openLinkTarget, pageRef, setError, setLinkPicker, snapshot, wikiTargets]);

  const handleOpenFile = useCallback((path: string) => {
    const resource = snapshot?.resources.find((entry) => entry.path === path);
    if (resource) void handleSelect(resource);
  }, [handleSelect, snapshot]);

  const updateWorkspaceSettings = useCallback((next: { capabilities: string[]; quickNoteDirectory: string }) => {
    const current = snapshotRef.current;
    if (!current) return;
    const optimistic = { ...current, capabilities: next.capabilities, defaults: { quickNoteDirectory: next.quickNoteDirectory } };
    snapshotRef.current = optimistic;
    setSnapshot(optimistic);
    if (inBrowser) return;
    if (workspaceSettingsTimerRef.current) window.clearTimeout(workspaceSettingsTimerRef.current);
    workspaceSettingsTimerRef.current = window.setTimeout(() => {
      const desired = snapshotRef.current;
      if (!desired) return;
      void updateWorkspaceManifest({ root: desired.root, enabledCapabilities: desired.capabilities,
        quickNoteDirectory: desired.defaults.quickNoteDirectory, baseRevision: current.manifestRevision })
        .then((updated) => { snapshotRef.current = updated; setSnapshot(updated); setStatusToast("Workspace settings saved"); })
        .catch(async (error) => {
          try {
            const reloaded = await invoke<WorkspaceSnapshot>("open_workspace", { path: current.root });
            snapshotRef.current = reloaded;
            setSnapshot(reloaded);
          } catch {
            snapshotRef.current = current;
            setSnapshot(current);
          }
          setError(String(error));
        });
    }, 180);
  }, [setError, setSnapshot, setStatusToast, snapshotRef]);

  return {
    createAndOpenPage, handleQuickNote, handleNewPage, handleUndo, handleImportCsv, handleNewTable,
    handleImportEditorAsset, handleOpenExternally, openLinkTarget, handleOpenWiki, handleOpenFile,
    updateWorkspaceSettings,
  };
}
