import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { demoCanvas, demoDataApp, demoPages, demoTextFiles, inBrowser } from "../demo";
import type { DataAppSnapshot } from "../data/types";
import { createDemoPageIO, createNativePageIO } from "../editor/pageIO";
import { readNativeCanvas } from "../canvas/adapter";
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
    onReplaceTab, onReplaceHistoryPath, refreshResources, onPageReady,
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

  const commitTitle = useCallback(async (title: string) => {
    const current = snapshotRef.current ?? snapshot;
    const resource = selectedRef.current;
    if (!current || !resource) return;
    const nextPath = renamedPath(resource.path, title);
    if (!title.trim() || nextPath === resource.path) {
      onTitle(fileTitle(resource.path));
      return;
    }
    const nextResource = { ...resource, path: nextPath };
    if (inBrowser) {
      setSnapshot((workspace) => workspace ? {
        ...workspace,
        resources: workspace.resources.map((entry) => entry.path === resource.path ? nextResource : entry),
      } : workspace);
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
      return;
    }
    onBusy(true);
    try {
      await invoke("rename_resource", { root: current.root, from: resource.path, to: nextPath });
      await refreshResources();
      setSelected(nextResource);
      selectedRef.current = nextResource;
      onReplaceTab(resource.path, nextResource);
      onReplaceHistoryPath(resource.path, nextPath);
      await handleSelect(nextResource, { recordHistory: false });
    } catch (error) {
      onError(String(error));
    } finally {
      onBusy(false);
    }
  }, [handleSelect, onBusy, onError, onReplaceHistoryPath, onReplaceTab, onTitle, refreshResources, setSnapshot, snapshot, snapshotRef]);

  return {
    selected, setSelected, session, setSession, pageRef, currentPageRevisionRef, reloadToken,
    handleSelect, reloadPageFromDisk, applyPageContent, saveLocalPage, openCreatedResource, clearSelection, clearSelectionIf,
    commitTitle, resetResources,
  };
}
