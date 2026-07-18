import { useCallback, type Dispatch, type MutableRefObject, type SetStateAction } from "react";

import { inBrowser } from "../demo";
import {
  createFolder,
  deleteResource,
  duplicateResource,
  moveResource,
} from "../lib/resourceMutations";
import {
  showNativeTreeFolderMenu,
  showNativeTreeResourceMenu,
} from "../lib/nativeMenus";
import { fileTimestamp } from "../lib/timestamp";
import { joinWorkspacePath, resourcePathExists, validateMoveResource } from "../lib/treeOps";
import type { Resource, WorkspaceSnapshot } from "../types";

function pageFileName(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  return trimmed.toLowerCase().endsWith(".md") ? trimmed : `${trimmed}.md`;
}

function folderName(raw: string): string {
  return raw.trim().replace(/^\/+|\/+$/g, "");
}

export interface TreeActionsOptions {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  setSnapshot: Dispatch<SetStateAction<WorkspaceSnapshot | null>>;
  setError: (message: string | null) => void;
  setBusy: (busy: boolean) => void;
  setStatusToast: (message: string | null) => void;
  setRevealPath: (path: string | null) => void;
  setInspectorOpen: (open: boolean) => void;
  refreshResources: () => Promise<void>;
  handleSelect: (resource: Resource, options?: { recordHistory?: boolean }) => Promise<void>;
  renameResource: (resource: Resource, title: string) => Promise<void>;
  clearSelectionIf: (path: string) => void;
  removeTabs: (predicate: (resource: Resource) => boolean) => void;
  createAndOpenPage: (
    relPath: string,
    options?: { templatePath?: string | null; title?: string | null; content?: string },
  ) => Promise<void>;
  requestTreeRename: (resource: Resource) => void;
  handleOpenExternally: (resource: Resource) => Promise<void>;
}

export function useTreeActionsController(options: TreeActionsOptions) {
  const {
    snapshot,
    snapshotRef,
    setSnapshot,
    setError,
    setBusy,
    setStatusToast,
    setRevealPath,
    setInspectorOpen,
    refreshResources,
    handleSelect,
    renameResource,
    clearSelectionIf,
    removeTabs,
    createAndOpenPage,
    requestTreeRename,
    handleOpenExternally,
  } = options;

  const copyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      setStatusToast(`Copied ${path}`);
      window.setTimeout(() => setStatusToast(null), 2200);
    } catch (error) {
      setError(String(error));
    }
  }, [setError, setStatusToast]);

  const handleDeleteResource = useCallback(async (resource: Resource) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (!workspace) return;
    const label = resource.path.split("/").pop() ?? resource.path;
    if (!window.confirm(`Delete “${label}”? This moves the resource to Trash.`)) return;

    if (inBrowser) {
      setSnapshot((current) => current ? {
        ...current,
        resources: current.resources.filter(
          (entry) => entry.path !== resource.path && !entry.path.startsWith(`${resource.path}/`),
        ),
      } : current);
      removeTabs((entry) => entry.path === resource.path || entry.path.startsWith(`${resource.path}/`));
      clearSelectionIf(resource.path);
      setError(null);
      return;
    }

    setBusy(true);
    try {
      await deleteResource(workspace.root, resource.path);
      removeTabs((entry) => entry.path === resource.path || entry.path.startsWith(`${resource.path}/`));
      clearSelectionIf(resource.path);
      await refreshResources();
      setError(null);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [clearSelectionIf, refreshResources, removeTabs, setBusy, setError, setSnapshot, snapshot, snapshotRef]);

  const handleDuplicateResource = useCallback(async (resource: Resource) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (!workspace) return;

    if (inBrowser) {
      const copyPath = `${resource.path.replace(/(\.[^./]+)?$/, " copy$1")}`;
      const duplicate: Resource = { ...resource, path: copyPath };
      setSnapshot((current) => current ? { ...current, resources: [...current.resources, duplicate] } : current);
      await handleSelect(duplicate);
      setError(null);
      return;
    }

    setBusy(true);
    try {
      const nextPath = await duplicateResource(workspace.root, resource.path);
      await refreshResources();
      const refreshed = snapshotRef.current ?? workspace;
      const duplicate = refreshed.resources.find((entry) => entry.path === nextPath);
      if (duplicate) await handleSelect(duplicate);
      setError(null);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [handleSelect, refreshResources, setBusy, setError, setSnapshot, snapshot, snapshotRef]);

  const handleMoveToFolder = useCallback(async (from: string, toDir: string) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (!workspace) return;
    const validation = validateMoveResource(from, toDir, workspace.resources);
    if (!validation.ok) {
      setStatusToast(validation.reason);
      window.setTimeout(() => setStatusToast(null), 2600);
      return;
    }

    if (inBrowser) {
      const destination = validation.destination;
      setSnapshot((current) => {
        if (!current) return current;
        return {
          ...current,
          resources: current.resources.map((entry) => {
            if (entry.path === from) return { ...entry, path: destination };
            if (entry.path.startsWith(`${from}/`)) {
              return { ...entry, path: destination + entry.path.slice(from.length) };
            }
            return entry;
          }),
        };
      });
      clearSelectionIf(from);
      setError(null);
      return;
    }

    setBusy(true);
    try {
      await moveResource(workspace.root, from, toDir);
      clearSelectionIf(from);
      await refreshResources();
      const refreshed = snapshotRef.current;
      const moved = refreshed?.resources.find((entry) => entry.path === validation.destination);
      if (moved) await handleSelect(moved, { recordHistory: false });
      setError(null);
    } catch (error) {
      setStatusToast(String(error));
      window.setTimeout(() => setStatusToast(null), 3200);
    } finally {
      setBusy(false);
    }
  }, [clearSelectionIf, handleSelect, refreshResources, setBusy, setError, setSnapshot, setStatusToast, snapshot, snapshotRef]);

  const handleNewPageInFolder = useCallback(async (folderPath: string) => {
    const name = window.prompt("New page name", `Untitled ${fileTimestamp()}.md`);
    const fileName = pageFileName(name ?? "");
    if (!fileName) return;
    const relPath = joinWorkspacePath(folderPath, fileName);
    const workspace = snapshotRef.current ?? snapshot;
    if (workspace && resourcePathExists(workspace.resources, relPath)) {
      setError(`${relPath} already exists`);
      return;
    }
    await createAndOpenPage(relPath);
  }, [createAndOpenPage, setError, snapshot, snapshotRef]);

  const handleNewFolderInFolder = useCallback(async (folderPath: string) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (!workspace) return;
    const name = window.prompt("New folder name");
    const trimmed = folderName(name ?? "");
    if (!trimmed) return;
    const relPath = joinWorkspacePath(folderPath, trimmed);
    if (resourcePathExists(workspace.resources, relPath)) {
      setError(`${relPath} already exists`);
      return;
    }

    if (inBrowser) {
      setSnapshot((current) => current ? {
        ...current,
        resources: [...current.resources, { path: relPath, kind: "folder" }],
      } : current);
      setRevealPath(relPath);
      setError(null);
      return;
    }

    setBusy(true);
    try {
      await createFolder(workspace.root, relPath);
      await refreshResources();
      setRevealPath(relPath);
      setError(null);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [refreshResources, setBusy, setError, setRevealPath, setSnapshot, snapshot, snapshotRef]);

  const handleTreeResourceContextMenu = useCallback((resource: Resource) => {
    void showNativeTreeResourceMenu({
      open: () => void handleSelect(resource),
      inspect: () => {
        void handleSelect(resource);
        setInspectorOpen(true);
      },
      openExternally: !inBrowser ? () => void handleOpenExternally(resource) : undefined,
      copyPath: () => void copyPath(resource.path),
      rename: () => requestTreeRename(resource),
      duplicate: () => void handleDuplicateResource(resource),
      delete: () => void handleDeleteResource(resource),
    });
  }, [
    copyPath,
    handleDeleteResource,
    handleDuplicateResource,
    handleOpenExternally,
    handleSelect,
    requestTreeRename,
    setInspectorOpen,
  ]);

  const handleTreeFolderContextMenu = useCallback((folderPath: string) => {
    void showNativeTreeFolderMenu({
      newPage: () => void handleNewPageInFolder(folderPath),
      newFolder: () => void handleNewFolderInFolder(folderPath),
      copyPath: () => void copyPath(folderPath),
    });
  }, [copyPath, handleNewFolderInFolder, handleNewPageInFolder]);

  const handleTreeRename = useCallback(async (resource: Resource, title: string) => {
    await renameResource(resource, title);
  }, [renameResource]);

  return {
    handleTreeResourceContextMenu,
    handleTreeFolderContextMenu,
    handleTreeRename,
    handleMoveToFolder,
    handleDeleteResource,
    handleDuplicateResource,
    handleNewPageInFolder,
    handleNewFolderInFolder,
    copyPath,
  };
}
