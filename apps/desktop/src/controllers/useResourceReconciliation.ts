import { useCallback, useEffect, useRef, useState, type MutableRefObject, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";
import { inBrowser } from "../demo";
import { isUnsaved, type SaveState } from "../editor/saveState";
import type { PageEditorHandle } from "../editor/PageEditor";
import { createPage } from "../lib/pages";
import type { OpenResourceSession } from "../resourceSession";
import type { Resource, WorkspaceChangeEvent } from "../types";
import { conflictSiblingPath, dispositionForModifiedResource, pathIsRemoved, shouldClearRenamedPath } from "./reconciliationPolicy";
import { listLinkRepairProposals } from "../lib/linkRepair";

export interface ExternalConflict {
  path: string;
}


type PageSession = Extract<OpenResourceSession, { kind: "page" }>;

export interface ResourceReconciliationOptions {
  snapshotRef: MutableRefObject<{ root: string } | null>;
  pageRef: MutableRefObject<PageSession | null>;
  currentPageRevisionRef: MutableRefObject<string | null>;
  getSelected: () => Resource | null;
  getSaveState: () => SaveState;
  pageEditorRef: RefObject<PageEditorHandle | null>;
  refreshResources: () => Promise<void>;
  handleWorkspaceUnavailable: (event: WorkspaceChangeEvent) => Promise<void>;
  reloadPageFromDisk: () => Promise<void>;
  applyPageContent: (raw: string, revision: string | null) => void;
  saveLocalPage: (raw: string) => Promise<void>;
  clearSelectionIf: (path: string) => void;
  removeTabs: (predicate: (resource: Resource) => boolean) => void;
  onError: (message: string | null) => void;
  setSaveStateIdle: () => void;
  onExternalLinkRepairProposal?: (proposalId: string, from: string, to: string) => void;
}

export interface ResourceReconciliationController {
  externalConflict: ExternalConflict | null;
  clearConflict: () => void;
  handleKeepIncoming: () => Promise<void>;
  handleKeepLocal: () => Promise<void>;
  handleKeepBoth: () => Promise<void>;
}

/** Owns watcher echoes, external resource revisions, and conflict actions.
 * Workspace lifecycle is injected so this hook never adopts or recreates a
 * workspace itself. */
export function useResourceReconciliation(options: ResourceReconciliationOptions): ResourceReconciliationController {
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const [externalConflict, setExternalConflict] = useState<ExternalConflict | null>(null);

  const clearConflict = useCallback(() => setExternalConflict(null), []);

  const handleWorkspaceChanged = useCallback(async (event: WorkspaceChangeEvent) => {
    const current = optionsRef.current;
    if (event.type === "workspace-unavailable" || (event.type === "deleted" && event.path === "lattice.yaml")) {
      await current.handleWorkspaceUnavailable(event);
      return;
    }

    await current.refreshResources();
    const selected = current.getSelected();
    const page = current.pageRef.current;

    if (event.type === "renamed") {
      if (shouldClearRenamedPath(page?.resource.path ?? "", event.from) || shouldClearRenamedPath(selected?.path ?? "", event.from)) {
        current.onError(`"${event.from}" was renamed to "${event.to}" outside Lattice.`);
        current.clearSelectionIf(event.from);
      }
      current.removeTabs((resource) => resource.path === event.from);
      const root = current.snapshotRef.current?.root;
      if (root && current.onExternalLinkRepairProposal) {
        try {
          const proposals = await listLinkRepairProposals(root);
          const match = proposals.find(
            (proposal) =>
              proposal.renameFrom === event.from
              && proposal.renameTo === event.to
              && proposal.source === "external-rename",
          );
          if (match) {
            current.onExternalLinkRepairProposal(match.id, event.from, event.to);
          }
        } catch (error) {
          current.onError(String(error));
        }
      }
      return;
    }

    if (event.type === "deleted") {
      if (page?.resource.path === event.path) {
        // Atomic replacement can emit remove before the stable replacement.
        try {
          const disk = await page.io.load();
          if (disk.revision === current.currentPageRevisionRef.current) return;
          if (isUnsaved(current.getSaveState())) {
            setExternalConflict({ path: event.path });
          } else {
            current.applyPageContent(disk.raw, disk.revision);
            current.setSaveStateIdle();
          }
          return;
        } catch {
          // Genuine deletion falls through to close/report below.
        }
      }
      if (selected && pathIsRemoved(selected.path, event.path)) {
        current.onError(`"${event.path}" was deleted outside Lattice.`);
        current.clearSelectionIf(event.path);
      }
      current.removeTabs((resource) => pathIsRemoved(resource.path, event.path));
      return;
    }

    if (event.type !== "modified") return;
    const disposition = dispositionForModifiedResource({
      eventPath: event.path,
      currentPath: page?.resource.path ?? null,
      eventRevision: event.revision,
      currentRevision: current.currentPageRevisionRef.current,
      unsaved: isUnsaved(current.getSaveState()),
    });
    if (disposition === "conflict") setExternalConflict({ path: event.path });
    else if (disposition === "reload") await current.reloadPageFromDisk();
  }, []);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listen<WorkspaceChangeEvent>("workspace-changed", (event) => {
      void handleWorkspaceChanged(event.payload);
    }).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten?.();
  }, [handleWorkspaceChanged]);

  const handleKeepIncoming = useCallback(async () => {
    await optionsRef.current.reloadPageFromDisk();
    setExternalConflict(null);
  }, []);

  const handleKeepLocal = useCallback(async () => {
    const current = optionsRef.current;
    const page = current.pageRef.current;
    const editor = current.pageEditorRef.current;
    if (!page || !editor) return;
    try {
      await current.saveLocalPage(editor.getRaw());
      current.setSaveStateIdle();
      setExternalConflict(null);
    } catch (error) {
      current.onError(String(error));
    }
  }, []);

  const handleKeepBoth = useCallback(async () => {
    const current = optionsRef.current;
    const page = current.pageRef.current;
    const editor = current.pageEditorRef.current;
    const root = current.snapshotRef.current?.root;
    if (!page || !editor || !root) return;
    try {
      await createPage({
        root,
        relPath: conflictSiblingPath(page.resource.path),
        content: editor.getRaw(),
      });
      await current.reloadPageFromDisk();
      setExternalConflict(null);
      await current.refreshResources();
    } catch (error) {
      current.onError(String(error));
    }
  }, []);

  return { externalConflict, clearConflict, handleKeepIncoming, handleKeepLocal, handleKeepBoth };
}
