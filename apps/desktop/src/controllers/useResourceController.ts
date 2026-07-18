import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { demoCanvas, demoDataApp, demoPages, demoTextFiles, inBrowser } from "../demo";
import type { DataAppSnapshot } from "../data/types";
import { createDemoPageIO, createNativePageIO } from "../editor/pageIO";
import { readNativeCanvas } from "../canvas/adapter";
import { previewLinkRepair, type LinkRepairPlan } from "../lib/linkRepair";
import { applyPathRemaps, type PathRemap } from "../lib/pathRemap";
import { moveResource } from "../lib/resourceMutations";
import { destinationPath } from "../lib/treeOps";
import type { OpenResourceSession } from "../resourceSession";
import { deriveResourceFormatId } from "../resourceRendererRegistry";
import type { Resource, WorkspaceSnapshot } from "../types";
import { createResourceLoadGate, isTextFormatId, loadTextResource, type ResourceLoadGate, type ResourceLoadTicket } from "./resourceLoad";

export interface ResourceControllerOptions {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  setSnapshot: Dispatch<SetStateAction<WorkspaceSnapshot | null>>;
  hasCapability: (capability: string) => boolean;
  onError: (message: string | null) => void;
  onBusy: (busy: boolean) => void;
  onActivity: (area: "files") => void;
  onTitle: (title: string) => void;
  onSelectionChanged: () => void;
  onRecordNavigation: (path: string) => void;
  onOpenTab: (resource: Resource) => void;
  onReplaceTab: (from: string, to: Resource) => void;
  onReplaceHistoryPath: (from: string, to: string) => void;
  refreshResources: () => Promise<void>;
  onPageReady: () => void;
  onLinkRepairReview: (review: {
    plan: LinkRepairPlan;
    from: string;
    to: string;
    mode: "lattice-rename" | "external";
    proposalId?: string;
  }) => Promise<"accepted" | "deferred" | "cancelled">;
}

export interface ResourceController {
  selected: Resource | null;
  setSelected: Dispatch<SetStateAction<Resource | null>>;
  session: OpenResourceSession | null;
  setSession: Dispatch<SetStateAction<OpenResourceSession | null>>;
  pageRef: MutableRefObject<Extract<OpenResourceSession, { kind: "page" }> | null>;
  currentPageRevisionRef: MutableRefObject<string | null>;
  reloadToken: number;
  handleSelect: (resource: Resource, options?: { recordHistory?: boolean }) => Promise<void>;
  reloadPageFromDisk: () => Promise<void>;
  applyPageContent: (raw: string, revision: string | null) => void;
  saveLocalPage: (raw: string) => Promise<void>;
  openCreatedResource: (resource: Resource, session: OpenResourceSession) => void;
  clearSelection: () => void;
  clearSelectionIf: (path: string) => void;
  commitTitle: (title: string) => Promise<void>;
  renameResource: (resource: Resource, title: string) => Promise<void>;
  moveResourceToFolder: (from: string, toDir: string) => Promise<void>;
  reconcilePathRemaps: (remaps: PathRemap[]) => Promise<void>;
  resetResources: () => void;
}

export function fileTitle(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.(md|canvas|pdf|png|jpe?g)$/i, "").replace(/\.data$/i, "");
}

export function renamedPath(path: string, title: string): string {
  const slash = path.lastIndexOf("/");
  const dir = slash >= 0 ? path.slice(0, slash + 1) : "";
  const base = slash >= 0 ? path.slice(slash + 1) : path;
  const dataSuffix = base.endsWith(".data") ? ".data" : "";
  const dot = dataSuffix ? -1 : base.lastIndexOf(".");
  const extension = dataSuffix || (dot > 0 ? base.slice(dot) : "");
  return `${dir}${title.trim()}${extension}`;
}

/** Owns selected resource identity, format-aware session loading, and page
 * title coordination. The abort ticket is deliberately local to this hook so
 * stale native reads cannot publish into a later renderer session. */
export function useResourceController(options: ResourceControllerOptions): ResourceController {
  const {
    snapshot, snapshotRef, setSnapshot, hasCapability, onError, onBusy,
    onActivity, onTitle, onSelectionChanged, onRecordNavigation, onOpenTab,
    onReplaceTab, onReplaceHistoryPath, refreshResources, onPageReady, onLinkRepairReview,
  } = options;
  const [selected, setSelected] = useState<Resource | null>(null);
  const [session, setSession] = useState<OpenResourceSession | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const pageRef = useRef<Extract<OpenResourceSession, { kind: "page" }> | null>(null);
  const selectedRef = useRef<Resource | null>(null);
  const sessionRef = useRef<OpenResourceSession | null>(null);
  const currentPageRevisionRef = useRef<string | null>(null);
  const loadGateRef = useRef<ResourceLoadGate>(createResourceLoadGate());

  useEffect(() => {
    selectedRef.current = selected;
    sessionRef.current = session;
    pageRef.current = session?.kind === "page" ? session : null;
  }, [selected, session]);

  const beginLoad = useCallback(() => {
    return loadGateRef.current.begin();
  }, []);

  const isCurrentLoad = useCallback((ticket: ResourceLoadTicket) => loadGateRef.current.isCurrent(ticket), []);

  const resetLoad = useCallback(() => {
    loadGateRef.current.cancel();
  }, []);

  const resetResources = useCallback(() => {
    resetLoad();
    selectedRef.current = null;
    sessionRef.current = null;
    pageRef.current = null;
    currentPageRevisionRef.current = null;
    setSelected(null);
    setSession(null);
    setReloadToken(0);
  }, [resetLoad]);

  const clearSelection = useCallback(() => {
    resetResources();
  }, [resetResources]);

  const clearSelectionIf = useCallback((path: string) => {
    const current = selectedRef.current;
    if (current && (current.path === path || current.path.startsWith(`${path}/`))) clearSelection();
  }, [clearSelection]);

  const openCreatedResource = useCallback((resource: Resource, nextSession: OpenResourceSession) => {
    resetLoad();
    selectedRef.current = resource;
    sessionRef.current = nextSession;
    pageRef.current = nextSession.kind === "page" ? nextSession : null;
    currentPageRevisionRef.current = nextSession.kind === "page" ? nextSession.revision : null;
    setSelected(resource);
    setSession(nextSession);
    setReloadToken((token) => token + 1);
    onOpenTab(resource);
    onActivity("files");
    onTitle(fileTitle(resource.path));
    onSelectionChanged();
  }, [onActivity, onOpenTab, onSelectionChanged, onTitle, resetLoad]);

  const handleSelect = useCallback(async (resource: Resource, selectionOptions: { recordHistory?: boolean } = {}) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (resource.kind === "folder") return;
    const ticket = beginLoad();
    onActivity("files");
    onOpenTab(resource);
    if (selectionOptions.recordHistory !== false) onRecordNavigation(resource.path);
    selectedRef.current = resource;
    sessionRef.current = null;
    pageRef.current = null;
    currentPageRevisionRef.current = null;
    setSelected(resource);
    onTitle(fileTitle(resource.path));
    onError(null);
    setSession(null);
    setReloadToken(0);
    onSelectionChanged();

    if (resource.kind === "canvas" && workspace) {
      if (!hasCapability("canvas")) {
        if (isCurrentLoad(ticket)) setSession({ kind: "unknown", resource });
        onError("Canvas is not enabled for this workspace.");
        return;
      }
      if (inBrowser) {
        if (isCurrentLoad(ticket)) setSession({ kind: "canvas", resource, json: demoCanvas, revision: "demo:0" });
        return;
      }
      onBusy(true);
      try {
        const canvas = await readNativeCanvas(workspace.root, resource.path);
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "canvas", resource, json: JSON.parse(canvas.content), revision: canvas.revision });
        }
      } catch (error) {
        if (isCurrentLoad(ticket)) onError(String(error));
      } finally {
        if (isCurrentLoad(ticket)) onBusy(false);
      }
      return;
    }

    if (resource.kind === "data-app" && workspace) {
      if (!hasCapability("sqlite")) {
        if (isCurrentLoad(ticket)) setSession({ kind: "unknown", resource });
        onError("Data apps are not enabled for this workspace.");
        return;
      }
      if (inBrowser) {
        if (isCurrentLoad(ticket)) setSession({ kind: "data-app", resource, snapshot: demoDataApp });
        return;
      }
      onBusy(true);
      try {
        const opened = await invoke<DataAppSnapshot>("open_data_app", { root: workspace.root, relPath: resource.path, viewName: null });
        if (isCurrentLoad(ticket)) setSession({ kind: "data-app", resource, snapshot: opened });
      } catch (error) {
        if (isCurrentLoad(ticket)) onError(String(error));
      } finally {
        if (isCurrentLoad(ticket)) onBusy(false);
      }
      return;
    }

    if (resource.kind === "file" && workspace) {
      const formatId = deriveResourceFormatId(resource);
      if (isTextFormatId(formatId)) {
        if (inBrowser) {
          const content = demoTextFiles[resource.path] ?? `# ${resource.path}\n\nBrowser demo text — no native filesystem access.\n`;
          const encoded = new TextEncoder().encode(content);
          if (isCurrentLoad(ticket)) {
            setSession({
              kind: "text",
              resource,
              inspection: {
                path: resource.path,
                kind: "file",
                profile: formatId === "file:json" || formatId === "json" ? "json" : formatId === "file:yaml" || formatId === "yaml" ? "yaml" : formatId === "file:code" || formatId === "code" ? "code" : "plain-text",
                capabilities: {
                  canInspect: true,
                  canReadRange: true,
                  canReadTextWindow: true,
                  canUpdate: false,
                  isText: true,
                  isBinary: false,
                  validatesStructure: false,
                  maxEditBytes: 0,
                },
                revision: "demo:0",
                size: encoded.length,
                isDirectory: false,
                encoding: "utf8",
                probeBytes: encoded.length,
                diagnostics: [],
              },
              content,
              revision: "demo:0",
              offset: 0,
              totalSize: encoded.length,
              truncated: false,
              encoding: "utf8",
              editable: false,
            });
          }
          return;
        }
        onBusy(true);
        try {
          const loaded = await loadTextResource(workspace.root, resource.path, ticket.controller.signal);
          if (isCurrentLoad(ticket)) {
            setSession({
              kind: "text",
              resource,
              inspection: loaded.inspection,
              content: loaded.window.content,
              revision: loaded.inspection.revision,
              offset: loaded.window.offset,
              totalSize: loaded.window.totalSize,
              truncated: loaded.window.truncated,
              encoding: loaded.window.encoding,
              editable: loaded.editable,
            });
          }
        } catch (error) {
          if (isCurrentLoad(ticket)) {
            setSession(null);
            onError(String(error));
          }
        } finally {
          if (isCurrentLoad(ticket)) onBusy(false);
        }
        return;
      }
      if (isCurrentLoad(ticket)) setSession({ kind: "unknown", resource });
      return;
    }

    if (resource.kind !== "page" || !workspace) return;
    onPageReady();
    if (inBrowser) {
      const content = demoPages[resource.path] ?? `# ${resource.path}\n`;
      if (isCurrentLoad(ticket)) {
        const next = { kind: "page" as const, resource, content, revision: "demo:0", io: createDemoPageIO(content) };
        sessionRef.current = next;
        pageRef.current = next;
        currentPageRevisionRef.current = next.revision;
        setSession(next);
      }
      return;
    }
    onBusy(true);
    try {
      const io = createNativePageIO(workspace.root, resource.path);
      const { raw, revision } = await io.load();
      if (isCurrentLoad(ticket)) {
        const next = { kind: "page" as const, resource, content: raw, revision, io };
        sessionRef.current = next;
        pageRef.current = next;
        currentPageRevisionRef.current = revision;
        setSession(next);
      }
    } catch (error) {
      if (isCurrentLoad(ticket)) {
        setSession(null);
        onError(String(error));
      }
    } finally {
      if (isCurrentLoad(ticket)) onBusy(false);
    }
  }, [beginLoad, hasCapability, isCurrentLoad, onActivity, onBusy, onError, onOpenTab, onPageReady, onRecordNavigation, onSelectionChanged, onTitle, resetLoad, snapshot, snapshotRef]);

  const reloadPageFromDisk = useCallback(async () => {
    const current = pageRef.current;
    if (!current) return;
    const ticket = beginLoad();
    try {
      const { raw, revision } = await current.io.load();
      if (!isCurrentLoad(ticket) || pageRef.current?.resource.path !== current.resource.path) return;
      const next = { ...current, content: raw, revision };
      pageRef.current = next;
      sessionRef.current = next;
      currentPageRevisionRef.current = revision;
      setSession((previous) => previous?.kind === "page" ? next : previous);
      setReloadToken((token) => token + 1);
      onPageReady();
    } catch (error) {
      if (isCurrentLoad(ticket)) onError(String(error));
    }
  }, [beginLoad, isCurrentLoad, onError, onPageReady]);

  const applyPageContent = useCallback((raw: string, revision: string | null) => {
    const current = pageRef.current;
    if (!current) return;
    const next = { ...current, content: raw, revision };
    pageRef.current = next;
    sessionRef.current = next;
    currentPageRevisionRef.current = revision;
    setSession((previous) => previous?.kind === "page" ? next : previous);
    setReloadToken((token) => token + 1);
  }, []);

  const saveLocalPage = useCallback(async (raw: string) => {
    const current = pageRef.current;
    if (!current) return;
    const disk = await current.io.load();
    const revision = await current.io.save(raw, disk.revision);
    applyPageContent(raw, revision);
  }, [applyPageContent]);

  const reconcileAfterPathChange = useCallback(async (
    from: string,
    to: string,
    fallbackResource?: Resource,
  ) => {
    const workspace = snapshotRef.current;
    const remappedSelectedPath = selectedRef.current
      ? applyPathRemaps(selectedRef.current.path, [{ from, to }])
      : null;
    const nextResource = workspace?.resources.find((entry) => entry.path === to)
      ?? fallbackResource
      ?? (selectedRef.current && remappedSelectedPath && remappedSelectedPath !== selectedRef.current.path
        ? { ...selectedRef.current, path: remappedSelectedPath }
        : null);

    if (nextResource) {
      onReplaceTab(from, nextResource);
    } else {
      onReplaceTab(from, { path: to, kind: "page" });
    }
    onReplaceHistoryPath(from, to);

    const selected = selectedRef.current;
    if (!selected || !remappedSelectedPath || remappedSelectedPath === selected.path) return;

    const resolved = workspace?.resources.find((entry) => entry.path === remappedSelectedPath)
      ?? (nextResource && nextResource.path === remappedSelectedPath
        ? nextResource
        : { ...selected, path: remappedSelectedPath });
    setSelected(resolved);
    selectedRef.current = resolved;
    onTitle(fileTitle(resolved.path));
    await handleSelect(resolved, { recordHistory: false });
  }, [handleSelect, onReplaceHistoryPath, onReplaceTab, onTitle, snapshotRef]);

  const reconcilePathRemaps = useCallback(async (remaps: PathRemap[]) => {
    if (remaps.length === 0) return;
    for (const remap of remaps) {
      const workspace = snapshotRef.current;
      const toResource = workspace?.resources.find((entry) => entry.path === remap.to);
      if (toResource) {
        onReplaceTab(remap.from, toResource);
      } else {
        onReplaceTab(remap.from, { path: remap.to, kind: "page" });
      }
      onReplaceHistoryPath(remap.from, remap.to);
    }

    const selected = selectedRef.current;
    if (!selected) return;
    const remapped = applyPathRemaps(selected.path, remaps);
    if (remapped === selected.path) return;
    const workspace = snapshotRef.current;
    const resolved = workspace?.resources.find((entry) => entry.path === remapped)
      ?? { ...selected, path: remapped };
    setSelected(resolved);
    selectedRef.current = resolved;
    onTitle(fileTitle(resolved.path));
    await handleSelect(resolved, { recordHistory: false });
  }, [handleSelect, onReplaceHistoryPath, onReplaceTab, onTitle, snapshotRef]);

  const renameResource = useCallback(async (resource: Resource, title: string) => {
    const current = snapshotRef.current ?? snapshot;
    if (!current) return;
    const nextPath = renamedPath(resource.path, title);
    if (!title.trim() || nextPath === resource.path) {
      if (selectedRef.current?.path === resource.path) onTitle(fileTitle(resource.path));
      return;
    }
    const nextResource = { ...resource, path: nextPath };
    if (inBrowser) {
      setSnapshot((workspace) => workspace ? {
        ...workspace,
        resources: workspace.resources.map((entry) => entry.path === resource.path ? nextResource : entry),
      } : workspace);
      if (selectedRef.current?.path === resource.path) {
        setSelected(nextResource);
        selectedRef.current = nextResource;
        onReplaceTab(resource.path, nextResource);
        onReplaceHistoryPath(resource.path, nextPath);
        if (sessionRef.current) {
          const nextSession = { ...sessionRef.current, resource: nextResource } as OpenResourceSession;
          sessionRef.current = nextSession;
          pageRef.current = nextSession.kind === "page" ? nextSession : null;
          setSession(nextSession);
        }
        onTitle(fileTitle(nextPath));
      }
      return;
    }
    onBusy(true);
    try {
      const plan = await previewLinkRepair(current.root, resource.path, nextPath, "lattice-rename");
      if (plan.candidates.length > 0) {
        const decision = await onLinkRepairReview({
          plan,
          from: resource.path,
          to: nextPath,
          mode: "lattice-rename",
        });
        if (decision === "cancelled") {
          if (selectedRef.current?.path === resource.path) onTitle(fileTitle(resource.path));
          return;
        }
      } else {
        await invoke("rename_resource", { root: current.root, from: resource.path, to: nextPath });
      }
      await refreshResources();
      await reconcileAfterPathChange(resource.path, nextPath, nextResource);
    } catch (error) {
      onError(String(error));
      if (selectedRef.current?.path === resource.path) onTitle(fileTitle(resource.path));
    } finally {
      onBusy(false);
    }
  }, [
    onBusy,
    onError,
    onLinkRepairReview,
    onReplaceHistoryPath,
    onReplaceTab,
    onTitle,
    reconcileAfterPathChange,
    refreshResources,
    setSnapshot,
    snapshot,
    snapshotRef,
  ]);

  /**
   * Move a resource into a folder. Link repair reuses rename-shaped from/to
   * full paths: when inbound links would break, the existing review modal runs
   * and `apply_link_repair` prepends ResourceRename(from, destination) — same
   * filesystem rename as ResourceMove, without double-applying a prior move.
   * Pure moves (no candidates) still use ResourceMove for honest history.
   */
  const moveResourceToFolder = useCallback(async (from: string, toDir: string) => {
    const current = snapshotRef.current ?? snapshot;
    if (!current) return;
    const destination = destinationPath(from, toDir);
    const resource = current.resources.find((entry) => entry.path === from);
    if (!resource || destination === from) return;
    const nextResource = { ...resource, path: destination };

    if (inBrowser) {
      setSnapshot((workspace) => {
        if (!workspace) return workspace;
        return {
          ...workspace,
          resources: workspace.resources.map((entry) => {
            if (entry.path === from) return { ...entry, path: destination };
            if (entry.path.startsWith(`${from}/`)) {
              return { ...entry, path: destination + entry.path.slice(from.length) };
            }
            return entry;
          }),
        };
      });
      await reconcileAfterPathChange(from, destination, nextResource);
      return;
    }

    onBusy(true);
    try {
      const plan = await previewLinkRepair(current.root, from, destination, "lattice-rename");
      if (plan.candidates.length > 0) {
        const decision = await onLinkRepairReview({
          plan,
          from,
          to: destination,
          mode: "lattice-rename",
        });
        if (decision === "cancelled") return;
      } else {
        await moveResource(current.root, from, toDir);
      }
      await refreshResources();
      const refreshed = snapshotRef.current;
      const moved = refreshed?.resources.find((entry) => entry.path === destination) ?? nextResource;
      await reconcileAfterPathChange(from, destination, moved);
    } catch (error) {
      onError(String(error));
    } finally {
      onBusy(false);
    }
  }, [
    onBusy,
    onError,
    onLinkRepairReview,
    reconcileAfterPathChange,
    refreshResources,
    setSnapshot,
    snapshot,
    snapshotRef,
  ]);

  const commitTitle = useCallback(async (title: string) => {
    const resource = selectedRef.current;
    if (!resource) return;
    await renameResource(resource, title);
  }, [renameResource]);

  return {
    selected, setSelected, session, setSession, pageRef, currentPageRevisionRef, reloadToken,
    handleSelect, reloadPageFromDisk, applyPageContent, saveLocalPage, openCreatedResource, clearSelection, clearSelectionIf,
    commitTitle, renameResource, moveResourceToFolder, reconcilePathRemaps, resetResources,
  };
}
