import { useCallback, useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { demoCanvas, demoDataApp, demoDataApps, demoNotebooks, demoPages, demoTextFiles, inBrowser } from "../demo";
import type { InterfaceSummary } from "../data/interfaces";
import type { DataAppSnapshot } from "../data/types";
import { createDemoPageIO, createNativePageIO } from "../editor/pageIO";
import { readNativeCanvas } from "../canvas/adapter";
import { previewBatchLinkRepair, previewLinkRepair, type BatchLinkRepairPlan, type LinkRepairPlan, type LinkRepairPathChange } from "../lib/linkRepair";
import { applyPathRemaps, type PathRemap } from "../lib/pathRemap";
import { moveResource, moveResources } from "../lib/resourceMutations";
import {
  isSyntheticResourceId,
  pathForResourceId,
  pathsForResourceIds,
  remapSelectedResourceIds,
  resourceIdForPathOrSynthetic,
  type CatalogEntry,
} from "../lib/resourceCatalog";
import { loadArtifactManifest } from "../lib/artifactRun";
import { loadDeckSession } from "../lib/deckRun";
import { loadDerivedManifest, loadDerivedStatus } from "../lib/derivedRun";
import { loadTaskManifest } from "../lib/taskRun";
import { loadWorkflow } from "../lib/workflowRun";
import { destinationPath } from "../lib/treeOps";
import type { PagePersistMode } from "../editor/collab/collabSession";
import type { OpenResourceSession } from "../resourceSession";
import { deriveResourceFormatId } from "../resourceRendererRegistry";
import type { Resource, WorkspaceSnapshot } from "../types";
import { createResourceLoadGate, isTextFormatId, loadTextResource, type ResourceLoadGate, type ResourceLoadTicket } from "./resourceLoad";

export type LinkRepairReviewRequest = {
  plan: LinkRepairPlan;
  from: string;
  to: string;
  mode: "lattice-rename" | "external";
  proposalId?: string;
  /** Present for multi-select moves (2+); destinations share `toDir`. */
  moves?: readonly LinkRepairPathChange[];
  toDir?: string;
  batchPlan?: BatchLinkRepairPlan;
};

export interface ResourceControllerOptions {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  setSnapshot: Dispatch<SetStateAction<WorkspaceSnapshot | null>>;
  /** Current id-keyed catalog for selection identity (path↔id). */
  getCatalog: () => ReadonlyMap<string, CatalogEntry>;
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
  /** Keep catalog in sync when browser demo mutates snapshot.resources directly. */
  seedCatalogFromResources: (resources: readonly Resource[]) => void;
  onPageReady: () => void;
  onLinkRepairReview: (review: LinkRepairReviewRequest) => Promise<"accepted" | "deferred" | "cancelled">;
}

export interface ResourceController {
  selected: Resource | null;
  selectedResourceIds: ReadonlySet<string>;
  /** Path projection of the current id selection (for delete/move call sites). */
  selectedPaths: ReadonlySet<string>;
  setSelected: Dispatch<SetStateAction<Resource | null>>;
  session: OpenResourceSession | null;
  setSession: Dispatch<SetStateAction<OpenResourceSession | null>>;
  pageRef: MutableRefObject<Extract<OpenResourceSession, { kind: "page" }> | null>;
  currentPageRevisionRef: MutableRefObject<string | null>;
  pagePersistModeRef: MutableRefObject<PagePersistMode>;
  reloadToken: number;
  handleSelect: (resource: Resource, options?: {
    recordHistory?: boolean;
    syncTreeSelection?: boolean;
    viewName?: string;
    interfaceDef?: InterfaceSummary;
    anchor?: string;
  }) => Promise<void>;
  applyTreeSelection: (detail: {
    resourceIds: ReadonlySet<string>;
    primary: Resource | null;
    open: boolean;
  }) => void;
  /** Remap selection when the catalog replaces synthetic ids or drops entries. */
  syncSelectionWithCatalog: (
    previous: ReadonlyMap<string, CatalogEntry>,
    next: ReadonlyMap<string, CatalogEntry>,
  ) => void;
  reloadPageFromDisk: () => Promise<void>;
  applyPageContent: (raw: string, revision: string | null) => void;
  saveLocalPage: (raw: string) => Promise<void>;
  openCreatedResource: (resource: Resource, session: OpenResourceSession) => void;
  clearSelection: () => void;
  clearSelectionIf: (path: string) => void;
  clearSelectionPaths: (paths: readonly string[]) => void;
  commitTitle: (title: string) => Promise<void>;
  renameResource: (resource: Resource, title: string) => Promise<void>;
  moveResourceToFolder: (from: string, toDir: string) => Promise<void>;
  moveResourcesToFolder: (fromPaths: readonly string[], toDir: string) => Promise<void>;
  reconcilePathRemaps: (remaps: PathRemap[]) => Promise<void>;
  resetResources: () => void;
}

function remapSelectedIdsForPathChanges(
  selected: ReadonlySet<string>,
  catalog: ReadonlyMap<string, CatalogEntry>,
  remaps: readonly PathRemap[],
): Set<string> {
  if (selected.size === 0 || remaps.length === 0) return new Set(selected);
  const next = new Set<string>();
  for (const id of selected) {
    if (catalog.has(id) && !isSyntheticResourceId(id)) {
      // Registry UUID survives renames; path alias updates in the catalog.
      next.add(id);
      continue;
    }
    if (catalog.has(id)) {
      next.add(id);
      continue;
    }
    if (isSyntheticResourceId(id)) {
      const oldPath = id.slice("path:".length);
      const newPath = applyPathRemaps(oldPath, remaps);
      next.add(resourceIdForPathOrSynthetic(catalog, newPath));
      continue;
    }
    const currentPath = pathForResourceId(catalog, id);
    if (currentPath) {
      next.add(id);
      continue;
    }
  }
  return next;
}

export function fileTitle(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.(md|canvas|pdf|png|jpe?g|ipynb)$/i, "").replace(/\.data$/i, "");
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
    snapshot, snapshotRef, setSnapshot, getCatalog, hasCapability, onError, onBusy,
    onActivity, onTitle, onSelectionChanged, onRecordNavigation, onOpenTab,
    onReplaceTab, onReplaceHistoryPath, refreshResources, seedCatalogFromResources, onPageReady, onLinkRepairReview,
  } = options;
  const [selected, setSelected] = useState<Resource | null>(null);
  const [selectedResourceIds, setSelectedResourceIds] = useState<ReadonlySet<string>>(() => new Set());
  const [session, setSession] = useState<OpenResourceSession | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const pageRef = useRef<Extract<OpenResourceSession, { kind: "page" }> | null>(null);
  const selectedRef = useRef<Resource | null>(null);
  const sessionRef = useRef<OpenResourceSession | null>(null);
  const currentPageRevisionRef = useRef<string | null>(null);
  const pagePersistModeRef = useRef<PagePersistMode>("plain");
  const loadGateRef = useRef<ResourceLoadGate>(createResourceLoadGate());

  const selectedPaths: ReadonlySet<string> = new Set(
    pathsForResourceIds(getCatalog(), selectedResourceIds),
  );

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
    pagePersistModeRef.current = "plain";
    setSelected(null);
    setSelectedResourceIds(new Set());
    setSession(null);
    setReloadToken(0);
  }, [resetLoad]);

  const clearSelection = useCallback(() => {
    resetResources();
  }, [resetResources]);

  const clearSelectionIf = useCallback((path: string) => {
    const catalog = getCatalog();
    setSelectedResourceIds((previous) => {
      const next = new Set<string>();
      for (const id of previous) {
        const entryPath = pathForResourceId(catalog, id);
        if (!entryPath) continue;
        if (entryPath === path || entryPath.startsWith(`${path}/`)) continue;
        next.add(id);
      }
      return next.size === previous.size ? previous : next;
    });
    const current = selectedRef.current;
    if (current && (current.path === path || current.path.startsWith(`${path}/`))) clearSelection();
  }, [clearSelection, getCatalog]);

  const clearSelectionPaths = useCallback((paths: readonly string[]) => {
    if (paths.length === 0) return;
    const doomed = new Set(paths);
    const catalog = getCatalog();
    setSelectedResourceIds((previous) => {
      const next = new Set<string>();
      for (const id of previous) {
        const entryPath = pathForResourceId(catalog, id);
        if (entryPath && doomed.has(entryPath)) continue;
        next.add(id);
      }
      return next.size === previous.size ? previous : next;
    });
    const current = selectedRef.current;
    if (current && doomed.has(current.path)) clearSelection();
  }, [clearSelection, getCatalog]);

  const openCreatedResource = useCallback((resource: Resource, nextSession: OpenResourceSession) => {
    resetLoad();
    selectedRef.current = resource;
    sessionRef.current = nextSession;
    pageRef.current = nextSession.kind === "page" ? nextSession : null;
    currentPageRevisionRef.current = nextSession.kind === "page" ? nextSession.revision : null;
    pagePersistModeRef.current = "plain";
    setSelected(resource);
    setSelectedResourceIds(new Set([resourceIdForPathOrSynthetic(getCatalog(), resource.path)]));
    setSession(nextSession);
    setReloadToken((token) => token + 1);
    onOpenTab(resource);
    onActivity("files");
    onTitle(fileTitle(resource.path));
    onSelectionChanged();
  }, [getCatalog, onActivity, onOpenTab, onSelectionChanged, onTitle, resetLoad]);

  const handleSelect = useCallback(async (
    resource: Resource,
    selectionOptions: {
      recordHistory?: boolean;
      syncTreeSelection?: boolean;
      viewName?: string;
      interfaceDef?: InterfaceSummary;
      anchor?: string;
    } = {},
  ) => {
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
    pagePersistModeRef.current = "plain";
    setSelected(resource);
    if (selectionOptions.syncTreeSelection !== false) {
      setSelectedResourceIds(new Set([resourceIdForPathOrSynthetic(getCatalog(), resource.path)]));
    }
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
      const viewName = selectionOptions.viewName ?? null;
      const interfaceDef = selectionOptions.interfaceDef;
      if (inBrowser) {
        if (isCurrentLoad(ticket)) {
          const base = demoDataApps[resource.path] ?? demoDataApp;
          const snapshot =
            viewName && base.available_views.includes(viewName)
              ? { ...base, active_view: viewName }
              : base;
          if (interfaceDef) {
            setSession({ kind: "interface", resource, snapshot, interfaceDef });
          } else {
            setSession({ kind: "data-app", resource, snapshot });
          }
        }
        return;
      }
      onBusy(true);
      try {
        const opened = await invoke<DataAppSnapshot>("open_data_app", {
          root: workspace.root,
          relPath: resource.path,
          viewName,
        });
        if (isCurrentLoad(ticket)) {
          if (interfaceDef) {
            setSession({ kind: "interface", resource, snapshot: opened, interfaceDef });
          } else {
            setSession({ kind: "data-app", resource, snapshot: opened });
          }
        }
      } catch (error) {
        if (isCurrentLoad(ticket)) onError(String(error));
      } finally {
        if (isCurrentLoad(ticket)) onBusy(false);
      }
      return;
    }

    if (resource.kind === "dataset" && workspace) {
      if (isCurrentLoad(ticket)) setSession({ kind: "dataset", resource });
      return;
    }

    if (resource.kind === "notebook" && workspace) {
      if (inBrowser) {
        const content = demoNotebooks[resource.path]
          ?? `{\n  "nbformat": 4,\n  "nbformat_minor": 5,\n  "metadata": {},\n  "cells": []\n}`;
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "notebook", resource, content, revision: "demo:0" });
        }
        return;
      }
      onBusy(true);
      try {
        const loaded = await loadTextResource(
          workspace.root,
          resource.path,
          ticket.controller.signal,
          { length: 10 * 1024 * 1024 },
        );
        if (isCurrentLoad(ticket)) {
          setSession({
            kind: "notebook",
            resource,
            content: loaded.window.content,
            revision: loaded.inspection.revision,
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

    if (resource.kind === "task" && workspace) {
      if (inBrowser) {
        if (isCurrentLoad(ticket)) {
          setSession({
            kind: "task",
            resource,
            manifest: {
              format: "lattice-task",
              version: 1,
              runtime: { type: "python", provider: "uv", project: "." },
              entrypoint: { command: ["python", "main.py"] },
              limits: { timeoutSeconds: 300 },
              inputs: [],
              outputs: [],
            },
          });
        }
        return;
      }
      onBusy(true);
      try {
        const manifest = await loadTaskManifest(workspace.root, resource.path);
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "task", resource, manifest });
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

    if (resource.kind === "workflow" && workspace) {
      if (inBrowser) {
        if (isCurrentLoad(ticket)) {
          setSession({
            kind: "workflow",
            resource,
            manifest: {
              format: "lattice-workflow",
              version: 1,
              name: resource.path,
              enabled: true,
              trigger: { type: "manual" },
              steps: [],
              rawYaml: "# Browser demo — open in native app to run workflows\n",
            },
          });
        }
        return;
      }
      onBusy(true);
      try {
        const manifest = await loadWorkflow(workspace.root, resource.path);
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "workflow", resource, manifest });
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

    if (resource.kind === "derived" && workspace) {
      if (inBrowser) {
        if (isCurrentLoad(ticket)) {
          setSession({
            kind: "derived",
            resource,
            manifest: {
              format: "lattice-derived-resource",
              version: 1,
              output: "./dist/index.html",
              inputs: ["./input.txt"],
              builderTask: "./Build.task/task.yaml",
              refreshMode: "on-demand",
            },
            status: null,
          });
        }
        return;
      }
      onBusy(true);
      try {
        const [manifest, status] = await Promise.all([
          loadDerivedManifest(workspace.root, resource.path),
          loadDerivedStatus(workspace.root, resource.path),
        ]);
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "derived", resource, manifest, status });
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

    if (resource.kind === "artifact" && workspace) {
      if (inBrowser) {
        if (isCurrentLoad(ticket)) {
          setSession({
            kind: "artifact",
            resource,
            manifest: {
              format: "lattice-artifact",
              version: 1,
              title: "Browser demo artifact",
              entrypoint: "./index.html",
              profile: "component",
              styles: [],
              bindings: {},
              permissions: { network: [], workspaceWrite: [] },
              fallback: { text: "Open in the native app to run sandboxed artifacts." },
              packagePath: resource.path,
            },
          });
        }
        return;
      }
      onBusy(true);
      try {
        const manifest = await loadArtifactManifest(workspace.root, resource.path);
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "artifact", resource, manifest });
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

    if (resource.kind === "deck" && workspace) {
      if (inBrowser) {
        if (isCurrentLoad(ticket)) {
          setSession({ kind: "deck", resource, initialSlideId: selectionOptions.anchor ?? null, deck: {
            format: "lattice-deck", version: 1, id: "browser-demo", title: "Deck preview",
            aspectRatio: "16:9", themeCss: "", start: "title", loop: false, durationMinutes: 20,
            packagePath: resource.path, slides: [{ id: "title", source: "slides/title.html", html: "<main class=\"lt-document lt-stack\"><h1>Deck preview</h1><p>Open in the native app for the package source.</p></main>", notes: "Browser demo notes.", transition: { type: "fade", durationMs: 280 } }],
          } });
        }
        return;
      }
      onBusy(true);
      try {
        const deck = await loadDeckSession(workspace.root, resource.path);
        if (isCurrentLoad(ticket)) setSession({ kind: "deck", resource, deck, initialSlideId: selectionOptions.anchor ?? null });
      } catch (error) {
        if (isCurrentLoad(ticket)) { setSession(null); onError(String(error)); }
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
  }, [beginLoad, getCatalog, hasCapability, isCurrentLoad, onActivity, onBusy, onError, onOpenTab, onPageReady, onRecordNavigation, onSelectionChanged, onTitle, resetLoad, snapshot, snapshotRef]);

  const applyTreeSelection = useCallback((detail: {
    resourceIds: ReadonlySet<string>;
    primary: Resource | null;
    open: boolean;
  }) => {
    setSelectedResourceIds(detail.resourceIds);
    if (detail.open && detail.primary) {
      void handleSelect(detail.primary, { syncTreeSelection: false });
    }
  }, [handleSelect]);

  const syncSelectionWithCatalog = useCallback((
    previous: ReadonlyMap<string, CatalogEntry>,
    next: ReadonlyMap<string, CatalogEntry>,
  ) => {
    setSelectedResourceIds((selected) => {
      if (selected.size === 0) return selected;
      const remapped = remapSelectedResourceIds(selected, previous, next);
      if (remapped.size === selected.size) {
        let unchanged = true;
        for (const id of selected) {
          if (!remapped.has(id)) {
            unchanged = false;
            break;
          }
        }
        if (unchanged) return selected;
      }
      return remapped;
    });
  }, []);

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

    setSelectedResourceIds((previous) =>
      remapSelectedIdsForPathChanges(previous, getCatalog(), [{ from, to }]),
    );

    const selected = selectedRef.current;
    if (!selected || !remappedSelectedPath || remappedSelectedPath === selected.path) return;

    const resolved = workspace?.resources.find((entry) => entry.path === remappedSelectedPath)
      ?? (nextResource && nextResource.path === remappedSelectedPath
        ? nextResource
        : { ...selected, path: remappedSelectedPath });
    setSelected(resolved);
    selectedRef.current = resolved;
    onTitle(fileTitle(resolved.path));
    await handleSelect(resolved, { recordHistory: false, syncTreeSelection: false });
  }, [getCatalog, handleSelect, onReplaceHistoryPath, onReplaceTab, onTitle, snapshotRef]);

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

    setSelectedResourceIds((previous) =>
      remapSelectedIdsForPathChanges(previous, getCatalog(), remaps),
    );

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
    await handleSelect(resolved, { recordHistory: false, syncTreeSelection: false });
  }, [getCatalog, handleSelect, onReplaceHistoryPath, onReplaceTab, onTitle, snapshotRef]);

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
      setSnapshot((workspace) => {
        if (!workspace) return workspace;
        const resources = workspace.resources.map((entry) =>
          entry.path === resource.path ? nextResource : entry,
        );
        seedCatalogFromResources(resources);
        return { ...workspace, resources };
      });
      setSelectedResourceIds((previous) =>
        remapSelectedIdsForPathChanges(previous, getCatalog(), [{ from: resource.path, to: nextPath }]),
      );
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
    getCatalog,
    onBusy,
    onError,
    onLinkRepairReview,
    onReplaceHistoryPath,
    onReplaceTab,
    onTitle,
    reconcileAfterPathChange,
    refreshResources,
    seedCatalogFromResources,
    setSnapshot,
    snapshot,
    snapshotRef,
  ]);

  /**
   * Move resources into a folder. Link repair reuses rename-shaped from/to
   * full paths: when inbound links would break, the review modal runs and
   * apply prepends ResourceRename(from, destination) — same filesystem rename
   * as ResourceMove, without double-applying a prior move. Pure moves (no
   * candidates) still use ResourceMove for honest history.
   *
   * Batch moves (2+) preview per-path repair, present one combined review when
   * any candidates exist, and apply N renames + unioned PageUpdates in one
   * transaction (one undo). Co-moved source pages are omitted from repair to
   * satisfy disjoint-path transaction rules.
   */
  const moveResourcesToFolder = useCallback(async (fromPaths: readonly string[], toDir: string) => {
    const current = snapshotRef.current ?? snapshot;
    if (!current) return;
    const unique = [...new Set(fromPaths.map((path) => path.trim()).filter(Boolean))];
    if (unique.length === 0) return;

    if (unique.length === 1) {
      const from = unique[0];
      const destination = destinationPath(from, toDir);
      const resource = current.resources.find((entry) => entry.path === from);
      if (!resource || destination === from) return;
      const nextResource = { ...resource, path: destination };

      if (inBrowser) {
        setSnapshot((workspace) => {
          if (!workspace) return workspace;
          const resources = workspace.resources.map((entry) => {
            if (entry.path === from) return { ...entry, path: destination };
            if (entry.path.startsWith(`${from}/`)) {
              return { ...entry, path: destination + entry.path.slice(from.length) };
            }
            return entry;
          });
          seedCatalogFromResources(resources);
          return { ...workspace, resources };
        });
        setSelectedResourceIds((previous) =>
          remapSelectedIdsForPathChanges(previous, getCatalog(), [{ from, to: destination }]),
        );
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
        setSelectedResourceIds((previous) =>
          remapSelectedIdsForPathChanges(previous, getCatalog(), [{ from, to: destination }]),
        );
        await reconcileAfterPathChange(from, destination, moved);
      } catch (error) {
        onError(String(error));
      } finally {
        onBusy(false);
      }
      return;
    }

    const remaps: PathRemap[] = unique.map((from) => ({
      from,
      to: destinationPath(from, toDir),
    }));
    const moves: LinkRepairPathChange[] = remaps.map((remap) => ({
      from: remap.from,
      to: remap.to,
    }));

    if (inBrowser) {
      setSnapshot((workspace) => {
        if (!workspace) return workspace;
        const resources = workspace.resources.map((entry) => {
          for (const remap of remaps) {
            if (entry.path === remap.from) return { ...entry, path: remap.to };
            if (entry.path.startsWith(`${remap.from}/`)) {
              return { ...entry, path: remap.to + entry.path.slice(remap.from.length) };
            }
          }
          return entry;
        });
        seedCatalogFromResources(resources);
        return { ...workspace, resources };
      });
      await reconcilePathRemaps(remaps);
      return;
    }

    onBusy(true);
    try {
      const batchPlan = await previewBatchLinkRepair(current.root, moves, "lattice-rename");
      if (batchPlan.candidates.length > 0) {
        const decision = await onLinkRepairReview({
          plan: {
            id: batchPlan.id,
            renameFrom: moves[0]?.from ?? "",
            renameTo: moves[0]?.to ?? "",
            source: batchPlan.source,
            candidates: batchPlan.candidates,
            createdAt: batchPlan.createdAt,
          },
          from: moves[0]?.from ?? "",
          to: moves[0]?.to ?? "",
          mode: "lattice-rename",
          moves,
          toDir,
          batchPlan,
        });
        if (decision === "cancelled") return;
      } else {
        await moveResources(current.root, unique, toDir);
      }
      await refreshResources();
      await reconcilePathRemaps(remaps);
    } catch (error) {
      onError(String(error));
    } finally {
      onBusy(false);
    }
  }, [
    getCatalog,
    onBusy,
    onError,
    onLinkRepairReview,
    reconcileAfterPathChange,
    reconcilePathRemaps,
    refreshResources,
    seedCatalogFromResources,
    setSnapshot,
    snapshot,
    snapshotRef,
  ]);

  const moveResourceToFolder = useCallback(async (from: string, toDir: string) => {
    await moveResourcesToFolder([from], toDir);
  }, [moveResourcesToFolder]);

  const commitTitle = useCallback(async (title: string) => {
    const resource = selectedRef.current;
    if (!resource) return;
    await renameResource(resource, title);
  }, [renameResource]);

  return {
    selected, selectedResourceIds, selectedPaths, setSelected, session, setSession, pageRef, currentPageRevisionRef, pagePersistModeRef, reloadToken,
    handleSelect, applyTreeSelection, syncSelectionWithCatalog, reloadPageFromDisk, applyPageContent, saveLocalPage, openCreatedResource,
    clearSelection, clearSelectionIf, clearSelectionPaths,
    commitTitle, renameResource, moveResourceToFolder, moveResourcesToFolder, reconcilePathRemaps, resetResources,
  };
}
