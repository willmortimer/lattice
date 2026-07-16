import { useCallback, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { demoCanvas, demoDataApp, demoPages, inBrowser } from "../demo";
import type { DataAppSnapshot } from "../data/types";
import { createDemoPageIO, createNativePageIO } from "../editor/pageIO";
import type { OpenResourceSession } from "../resourceSession";
import type { Resource, WorkspaceSnapshot } from "../types";

export interface ResourceControllerOptions {
  snapshot: WorkspaceSnapshot | null;
  snapshotRef: MutableRefObject<WorkspaceSnapshot | null>;
  settings: { performance: { maxOpenTabs: number } };
  hasCapability: (capability: string) => boolean;
  onError: (message: string | null) => void;
  onBusy: (busy: boolean) => void;
  onActivity: (area: "files") => void;
  onTitle: (title: string) => void;
  onResetSelection: () => void;
  onRecordNavigation: (path: string) => void;
  onPageReady: () => void;
}

export interface ResourceController {
  selected: Resource | null;
  setSelected: Dispatch<SetStateAction<Resource | null>>;
  session: OpenResourceSession | null;
  setSession: Dispatch<SetStateAction<OpenResourceSession | null>>;
  openTabs: Resource[];
  setOpenTabs: Dispatch<SetStateAction<Resource[]>>;
  handleSelect: (resource: Resource, options?: { recordHistory?: boolean }) => Promise<void>;
  resetResources: () => void;
}

/** Owns resource identity, tabs, and the bounded async load for each native
 * surface. Shell concerns enter only through typed callbacks. */
export function useResourceController(options: ResourceControllerOptions): ResourceController {
  const {
    snapshot, snapshotRef, settings, hasCapability, onError, onBusy, onActivity,
    onTitle, onResetSelection, onRecordNavigation, onPageReady,
  } = options;
  const [selected, setSelected] = useState<Resource | null>(null);
  const [session, setSession] = useState<OpenResourceSession | null>(null);
  const [openTabs, setOpenTabs] = useState<Resource[]>([]);

  const resetResources = useCallback(() => {
    setSelected(null);
    setSession(null);
    setOpenTabs([]);
  }, []);

  const handleSelect = useCallback(async (resource: Resource, selectionOptions: { recordHistory?: boolean } = {}) => {
    const workspace = snapshotRef.current ?? snapshot;
    if (resource.kind === "folder") return;
    onActivity("files");
    setOpenTabs((tabs) => tabs.some((tab) => tab.path === resource.path)
      ? tabs
      : [...tabs, resource].slice(-settings.performance.maxOpenTabs));
    if (selectionOptions.recordHistory !== false) onRecordNavigation(resource.path);
    setSelected(resource);
    onTitle(resource.path.split("/").pop()?.replace(/\.(md|canvas|pdf|png|jpe?g)$/i, "").replace(/\.data$/i, "") ?? resource.path);
    onError(null);
    setSession(null);
    onResetSelection();

    if (resource.kind === "canvas" && workspace) {
      if (!hasCapability("canvas")) {
        setSession({ kind: "unknown", resource });
        onError("Canvas is not enabled for this workspace.");
        return;
      }
      if (inBrowser) {
        setSession({ kind: "canvas", resource, json: demoCanvas });
        return;
      }
      onBusy(true);
      try {
        const content = await invoke<string>("read_file", { root: workspace.root, relPath: resource.path });
        setSession({ kind: "canvas", resource, json: JSON.parse(content) });
      } catch (error) {
        onError(String(error));
      } finally {
        onBusy(false);
      }
      return;
    }

    if (resource.kind === "data-app" && workspace) {
      if (!hasCapability("sqlite")) {
        setSession({ kind: "unknown", resource });
        onError("Data apps are not enabled for this workspace.");
        return;
      }
      if (inBrowser) {
        setSession({ kind: "data-app", resource, snapshot: demoDataApp });
        return;
      }
      onBusy(true);
      try {
        const opened = await invoke<DataAppSnapshot>("open_data_app", { root: workspace.root, relPath: resource.path, viewName: null });
        setSession({ kind: "data-app", resource, snapshot: opened });
      } catch (error) {
        onError(String(error));
      } finally {
        onBusy(false);
      }
      return;
    }

    if (resource.kind !== "page" || !workspace) return;
    onPageReady();
    if (inBrowser) {
      const content = demoPages[resource.path] ?? `# ${resource.path}\n`;
      setSession({ kind: "page", resource, content, revision: "demo:0", io: createDemoPageIO(content) });
      return;
    }
    onBusy(true);
    try {
      const io = createNativePageIO(workspace.root, resource.path);
      const { raw, revision } = await io.load();
      setSession({ kind: "page", resource, content: raw, revision, io });
    } catch (error) {
      setSession(null);
      onError(String(error));
    } finally {
      onBusy(false);
    }
  }, [hasCapability, onActivity, onBusy, onError, onPageReady, onRecordNavigation, onResetSelection, onTitle, settings.performance.maxOpenTabs, snapshot, snapshotRef]);

  return { selected, setSelected, session, setSession, openTabs, setOpenTabs, handleSelect, resetResources };
}
