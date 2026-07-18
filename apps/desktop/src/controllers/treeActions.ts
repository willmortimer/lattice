import { useCallback, type Dispatch, type MutableRefObject, type SetStateAction } from "react";

import { inBrowser } from "../demo";
import {
  createFolder,
  deleteResources,
  duplicateResource,
} from "../lib/resourceMutations";
import {
  showNativeTreeFolderMenu,
  showNativeTreeResourceMenu,
} from "../lib/nativeMenus";
import { fileTimestamp } from "../lib/timestamp";
import { joinWorkspacePath, resourcePathExists, validateMoveResources } from "../lib/treeOps";
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
  moveResourcesToFolder: (fromPaths: readonly string[], toDir: string) => Promise<void>;
  selectedPaths: ReadonlySet<string>;
  clearSelectionPaths: (paths: readonly string[]) => void;
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
    moveResourcesToFolder,
    selectedPaths,
    clearSelectionPaths,
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

  const handleDeleteResources = useCallback(async (paths: readonly string[]) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (!workspace) return;
    const unique = [...new Set(paths.map((path) => path.trim()).filter(Boolean))];
    if (unique.length === 0) return;

    const label = unique.length === 1
      ? (unique[0].split("/").pop() ?? unique[0])
      : `${unique.length} resources`;
    if (!window.confirm(`Delete “${label}”? This moves ${unique.length === 1 ? "the resource" : "them"} to Trash.`)) {
      return;
    }

    if (inBrowser) {
      const doomed = new Set(unique);
      setSnapshot((current) => current ? {
        ...current,
        resources: current.resources.filter((entry) => {
          for (const path of doomed) {
            if (entry.path === path || entry.path.startsWith(`${path}/`)) return false;
          }
          return true;
        }),
      } : current);
      removeTabs((entry) => {
        for (const path of doomed) {
          if (entry.path === path || entry.path.startsWith(`${path}/`)) return true;
        }
        return false;
      });
      clearSelectionPaths(unique);
      setError(null);
      return;
    }

    setBusy(true);
    try {
      await deleteResources(workspace.root, unique);
      removeTabs((entry) => {
        for (const path of unique) {
          if (entry.path === path || entry.path.startsWith(`${path}/`)) return true;
        }
        return false;
      });
      clearSelectionPaths(unique);
      await refreshResources();
      setError(null);
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  }, [clearSelectionPaths, refreshResources, removeTabs, setBusy, setError, setSnapshot, snapshot, snapshotRef]);

  const handleDeleteResource = useCallback(async (resource: Resource) => {
    const batch = selectedPaths.has(resource.path) && selectedPaths.size > 1
      ? [...selectedPaths]
      : [resource.path];
    await handleDeleteResources(batch);
  }, [handleDeleteResources, selectedPaths]);

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

  const handleMoveToFolder = useCallback(async (fromPaths: readonly string[], toDir: string) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (!workspace) return;
    const validation = validateMoveResources(fromPaths, toDir, workspace.resources);
    if (!validation.ok) {
      setStatusToast(validation.reason);
      window.setTimeout(() => setStatusToast(null), 2600);
      return;
    }

    try {
      await moveResourcesToFolder(fromPaths, toDir);
      setError(null);
    } catch (error) {
      setStatusToast(String(error));
      window.setTimeout(() => setStatusToast(null), 3200);
    }
  }, [moveResourcesToFolder, setError, setStatusToast, snapshot, snapshotRef]);

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
    handleDeleteResources,
    handleDuplicateResource,
    handleNewPageInFolder,
    handleNewFolderInFolder,
    copyPath,
  };
}
