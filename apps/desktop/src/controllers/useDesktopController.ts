import { demoSnapshot, demoStartEmpty, inBrowser } from "../demo";
import type { SaveState } from "../editor/saveState";
import type { PageEditorHandle } from "../editor/PageEditor";
import { saveResourceTreeCollapsed, saveSidebarWidth, type DesktopSession } from "../lib/profile";
import {
  collapsedPathsForWorkspace,
  serializeResourceTreeCollapseState,
  updateCollapsedPathsForWorkspace,
  type ResourceTreeCollapseState,
} from "../lib/treeCollapse";
import { demoLinkTargets, type ResourceLinkTarget } from "../lib/resourceLinks";
import {
  applyLinkRepair,
  applyLinkRepairProposal,
  deferLinkRepairProposal,
  getLinkRepairProposal,
  type LinkRepairPlan,
} from "../lib/linkRepair";
import { installNativeContextMenus } from "../lib/nativeMenus";
import { QUICK_NOTE_SHORTCUT, showQuickNote } from "../quickNoteWindow";
import { applyResolvedTheme, loadThemeCatalog, setAppearanceMode, setFixedTheme, startThemeWatch, type ThemeCatalogPayload, type ThemeSummaryPayload } from "../theme";
import type { Resource, WorkspaceSnapshot } from "../types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { matchesKeybinding, useAppSettings } from "../settings/model";
import { useNavigationController } from "./useNavigationController";
import { useResourceController } from "./useResourceController";
import { useResourceReconciliation, type ResourceReconciliationController } from "./useResourceReconciliation";
import { useWorkspaceController } from "./useWorkspaceController";
import { useDesktopActionsController } from "./desktopActions";
import { useTreeActionsController } from "./treeActions";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import type { PaletteItem } from "../CommandPalette";
export function useDesktopController() {
  const {
    profile,
    ready: profileReady,
    settings,
    startup,
    recents,
    diagnostics: profileDiagnostics,
    saveError: profileSaveError,
    setSettings,
    setStartup,
    rememberWorkspace,
    clearRecents,
    removeRecent,
    refreshProfile,
    resetSettings,
  } = useAppSettings();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });
  const [newWorkspaceOpen, setNewWorkspaceOpen] = useState(false);
  const [statusToast, setStatusToast] = useState<string | null>(null);
  const [runtimeNotice, setRuntimeNotice] = useState<{
    code: string; title: string; message: string; path: string | null;
  } | null>(null);
  const [dismissedNoticeCodes, setDismissedNoticeCodes] = useState<string[]>([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [searchPaneOpen, setSearchPaneOpen] = useState(false);
  const [themeCatalog, setThemeCatalog] = useState<ThemeCatalogPayload | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(272);
  const [resourceTreeCollapsedByWorkspace, setResourceTreeCollapsedByWorkspace] =
    useState<ResourceTreeCollapseState>({});
  const [revealPath, setRevealPath] = useState<string | null>(null);
  const [linkPicker, setLinkPicker] = useState<{
    query: string; candidates: ResourceLinkTarget[];
  } | null>(null);
  const [linkRepairReview, setLinkRepairReview] = useState<{
    plan: LinkRepairPlan;
    from: string;
    to: string;
    mode: "lattice-rename" | "external";
    proposalId?: string;
  } | null>(null);
  const linkRepairResolverRef = useRef<
    ((result: "accepted" | "deferred" | "cancelled") => void) | null
  >(null);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [treeRenameRequest, setTreeRenameRequest] = useState<{ path: string; token: number } | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const pageEditorRef = useRef<PageEditorHandle>(null);
  const resourceResetRef = useRef<() => void>(() => undefined);
  const resourceSelectRef = useRef<(resource: Resource, options?: { recordHistory?: boolean }) => Promise<void>>(async () => undefined);
  const resourceClearRef = useRef<() => void>(() => undefined);
  const selectedRef = useRef<Resource | null>(null);
  const saveStateRef = useRef(saveState);
  saveStateRef.current = saveState;
  const workspaceSnapshotRef = useRef<WorkspaceSnapshot | null>(inBrowser && !demoStartEmpty ? demoSnapshot : null);
  const reconciliationRef = useRef<ResourceReconciliationController>({
    externalConflict: null,
    clearConflict: () => undefined,
    handleKeepIncoming: async () => undefined,
    handleKeepLocal: async () => undefined,
    handleKeepBoth: async () => undefined,
  });

  const navigationController = useNavigationController({
    maxOpenTabs: settings.performance.maxOpenTabs,
    getResource: (path) => workspaceSnapshotRef.current?.resources.find((resource) => resource.path === path) ?? null,
    getSelectedPath: () => selectedRef.current?.path ?? null,
    canCloseTab: (path) => path !== selectedRef.current?.path || !saveStateRef.current ||
      !(saveStateRef.current.status === "dirty" || saveStateRef.current.status === "saving" || saveStateRef.current.status === "conflict") ||
      !settings.files.confirmCloseWithUnsavedChanges || window.confirm("Close this tab with unsaved changes?"),
    onSelect: (resource, options) => resourceSelectRef.current(resource, options),
    onClearSelection: () => resourceClearRef.current(),
  });
  const {
    state: navigation,
    activityArea,
    setActivityArea,
    openTabs,
  } = navigationController;
  const resetNavigation = navigationController.reset;
  const getNavigationSessionState = navigationController.getSessionState;
  const restoreNavigationSession = navigationController.restoreSession;

  useEffect(() => installNativeContextMenus(() => settingsRef.current.diagnostics.nativeContextMenus), []);
  useEffect(() => {
    document.documentElement.dataset.motion = settings.performance.reducedMotion;
  }, [settings.performance.reducedMotion]);
  useEffect(() => {
    if (profile.sidebarWidth && profile.sidebarWidth >= 210 && profile.sidebarWidth <= 480) {
      setSidebarWidth(profile.sidebarWidth);
    }
  }, [profile.sidebarWidth]);
  useEffect(() => {
    setResourceTreeCollapsedByWorkspace(profile.resourceTreeCollapsedByWorkspace ?? {});
  }, [profile.resourceTreeCollapsedByWorkspace]);
  useEffect(() => {
    const messages = [
      ...profileDiagnostics.map((diagnostic) => `${diagnostic.path}: ${diagnostic.message}`),
      ...(profileSaveError ? [profileSaveError] : []),
    ];
    if (messages.length > 0) setError(messages.join("\n"));
  }, [profileDiagnostics, profileSaveError]);

  const onAdopt = useCallback(async () => {
    resourceResetRef.current();
    setSaveState({ status: "idle" });
    reconciliationRef.current.clearConflict();
    setRuntimeNotice(null);
    resetNavigation();
  }, [resetNavigation]);

  const onWorkspaceUnavailable = useCallback(async (root: string) => {
    resourceResetRef.current();
    resetNavigation();
    reconciliationRef.current.clearConflict();
    setSaveState({ status: "idle" });
    setRuntimeNotice({
      code: "open-workspace-unavailable",
      title: "Workspace unavailable",
      message:
        "The open workspace was moved or deleted outside Lattice. It was closed without recreating any content; create a workspace or open its new location.",
      path: root,
    });
  }, [resetNavigation]);

  const getSession = useCallback((): Omit<DesktopSession, "root"> => {
    const current = getNavigationSessionState();
    return {
      tabs: current.tabs.map((tab) => tab.path),
      active: selectedRef.current?.path ?? null,
      activity: current.activity,
      inspector: inspectorOpen,
    };
  }, [getNavigationSessionState, inspectorOpen]);

  const restoreSession = useCallback((stored: DesktopSession, workspace: WorkspaceSnapshot) => {
    const tabs = (stored.tabs ?? [])
      .map((path) => workspace.resources.find((resource) => resource.path === path))
      .filter((resource): resource is Resource => Boolean(resource));
    restoreNavigationSession({
      tabs,
      activity: (stored.activity as import("./useNavigationController").ActivityArea | null) ?? (tabs.length > 0 ? "files" : "home"),
    });
    setInspectorOpen(Boolean(stored.inspector));
    const active = workspace.resources.find((resource) => resource.path === stored.active) ?? tabs[0] ?? null;
    if (active) void resourceSelectRef.current(active, { recordHistory: false });
  }, [restoreNavigationSession]);

  const workspaceController = useWorkspaceController({
    initialSnapshot: inBrowser && !demoStartEmpty ? demoSnapshot : null,
    profile,
    profileReady,
    startup,
    recents,
    demoStartEmpty,
    setError,
    setBusy,
    setStatusToast,
    setNewWorkspaceOpen,
    rememberWorkspace,
    removeRecent,
    refreshProfile,
    getSession,
    restoreSession,
    onAdopt,
    onWorkspaceUnavailable,
    openResource: (resource) => resourceSelectRef.current(resource),
  });
  const { snapshot, snapshotRef, setSnapshot, workspacesDir, templates, adoptWorkspace,
    handleGetStarted, handleOpenWorkspace, openRecent, handleCreateWorkspace,
    openNewWorkspaceDialog, pickWorkspaceFolder, refreshResources } = workspaceController;
  workspaceSnapshotRef.current = snapshot;
  useEffect(() => {
    workspaceSnapshotRef.current = snapshot;
  }, [snapshot]);
  const assetRoot = inBrowser ? null : snapshot?.root ?? null;
  const wikiTargets = useMemo(() => demoLinkTargets(snapshot?.resources ?? []), [snapshot?.resources]);
  const hasCapability = useCallback(
    (capability: string) => capability === "pages" || Boolean(snapshot?.capabilities.includes(capability)),
    [snapshot?.capabilities],
  );

  const onLinkRepairReview = useCallback((review: {
    plan: LinkRepairPlan;
    from: string;
    to: string;
    mode: "lattice-rename" | "external";
    proposalId?: string;
  }) => new Promise<"accepted" | "deferred" | "cancelled">((resolve) => {
    linkRepairResolverRef.current = resolve;
    setLinkRepairReview(review);
  }), []);

  const finishLinkRepairReview = useCallback((result: "accepted" | "deferred" | "cancelled") => {
    linkRepairResolverRef.current?.(result);
    linkRepairResolverRef.current = null;
    setLinkRepairReview(null);
  }, []);

  const handleLinkRepairAccept = useCallback(async (acceptedCandidateIds: string[]) => {
    const review = linkRepairReview;
    const root = snapshotRef.current?.root;
    if (!review || !root) return;
    setBusy(true);
    try {
      if (review.mode === "lattice-rename") {
        await applyLinkRepair(root, review.from, review.to, acceptedCandidateIds, review.plan);
      } else if (review.proposalId) {
        await applyLinkRepairProposal(root, review.proposalId, acceptedCandidateIds);
      }
      finishLinkRepairReview("accepted");
    } catch (err) {
      setError(String(err));
      finishLinkRepairReview("cancelled");
    } finally {
      setBusy(false);
    }
  }, [finishLinkRepairReview, linkRepairReview]);

  const handleLinkRepairDefer = useCallback(async () => {
    const review = linkRepairReview;
    const root = snapshotRef.current?.root;
    if (!review || !root) return;
    setBusy(true);
    try {
      if (review.mode === "lattice-rename") {
        await deferLinkRepairProposal(root, review.plan);
        await invoke("rename_resource", { root, from: review.from, to: review.to });
      }
      finishLinkRepairReview("deferred");
    } catch (err) {
      setError(String(err));
      finishLinkRepairReview("cancelled");
    } finally {
      setBusy(false);
    }
  }, [finishLinkRepairReview, linkRepairReview]);

  const openExternalLinkRepairProposal = useCallback(async (proposalId: string, from: string, to: string) => {
    const root = snapshotRef.current?.root;
    if (!root) return;
    try {
      const plan = await getLinkRepairProposal(root, proposalId);
      await onLinkRepairReview({ plan, from, to, mode: "external", proposalId });
    } catch (err) {
      setError(String(err));
    }
  }, [onLinkRepairReview]);

  const resourceController = useResourceController({
    snapshot,
    snapshotRef,
    setSnapshot,
    hasCapability,
    onError: setError,
    onBusy: setBusy,
    onActivity: navigationController.setActivityArea,
    onTitle: (title) => { setTitleDraft(title); setEditingTitle(false); },
    onSelectionChanged: () => reconciliationRef.current.clearConflict(),
    onRecordNavigation: navigationController.record,
    onOpenTab: navigationController.openTab,
    onReplaceTab: navigationController.replaceTab,
    onReplaceHistoryPath: navigationController.replacePath,
    refreshResources,
    onPageReady: () => setSaveState({ status: "idle" }),
    onLinkRepairReview,
  });
  resourceResetRef.current = resourceController.resetResources;
  resourceSelectRef.current = resourceController.handleSelect;
  resourceClearRef.current = resourceController.clearSelection;
  const { selected, session, pageRef, currentPageRevisionRef, reloadToken, handleSelect } = resourceController;
  const page = session?.kind === "page" ? session : null;
  useEffect(() => { selectedRef.current = selected; }, [selected]);
  const profileNotices = [runtimeNotice, ...profile.notices]
    .filter((notice): notice is NonNullable<typeof notice> => notice !== null)
    .filter((notice) => !dismissedNoticeCodes.includes(notice.code));
  const applyThemeCatalog = useCallback((catalog: ThemeCatalogPayload) => {
    setThemeCatalog(catalog);
    applyResolvedTheme(catalog.resolved);
    const diags = [...catalog.diagnostics, ...catalog.resolved.diagnostics].filter(
      (diagnostic, index, all) =>
        all.findIndex(
          (candidate) =>
            candidate.path === diagnostic.path && candidate.message === diagnostic.message,
        ) === index,
    );
    if (diags.length > 0) {
      setError(diags.map((d) => `${d.path}: ${d.message}`).join("\n"));
    }
  }, []);

  const reloadTheme = useCallback(async () => {
    try {
      const catalog = await loadThemeCatalog(snapshotRef.current?.root);
      applyThemeCatalog(catalog);
    } catch (err) {
      setError(String(err));
    }
  }, [applyThemeCatalog]);

  // Initial theme resolve + re-apply when the open workspace changes.
  useEffect(() => {
    void reloadTheme();
  }, [reloadTheme, snapshot?.root]);

  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      stop = await startThemeWatch(snapshot?.root ?? null, () => {
        if (!cancelled) void reloadTheme();
      });
    })();
    return () => {
      cancelled = true;
      stop?.();
    };
  }, [snapshot?.root, reloadTheme]);

  const reconciliationController = useResourceReconciliation({
    snapshotRef,
    pageRef,
    currentPageRevisionRef,
    getSelected: () => selectedRef.current,
    getSaveState: () => saveStateRef.current,
    pageEditorRef,
    refreshResources,
    handleWorkspaceUnavailable: workspaceController.handleWorkspaceChanged,
    reloadPageFromDisk: resourceController.reloadPageFromDisk,
    applyPageContent: resourceController.applyPageContent,
    saveLocalPage: resourceController.saveLocalPage,
    clearSelectionIf: resourceController.clearSelectionIf,
    removeTabs: navigationController.removeTabs,
    onError: setError,
    setSaveStateIdle: () => setSaveState({ status: "idle" }),
    onExternalLinkRepairProposal: (proposalId, from, to) => {
      void openExternalLinkRepairProposal(proposalId, from, to);
    },
  });
  reconciliationRef.current = reconciliationController;
  const { externalConflict, handleKeepIncoming, handleKeepLocal, handleKeepBoth } = reconciliationController;
  const actions = useDesktopActionsController({
    snapshot, snapshotRef, setSnapshot, selected, pageRef, wikiTargets,
    setError, setBusy, setStatusToast, setSaveStateIdle: () => setSaveState({ status: "idle" }),
    setActivityArea: (area) => navigationController.setActivityArea(area),
    setRevealPath, setLinkPicker, refreshResources, handleSelect,
    openCreatedResource: resourceController.openCreatedResource,
  });
  const {
    handleQuickNote, handleNewPage, handleNewTable, handleImportCsv, handleUndo,
    handleOpenExternally, handleOpenFile, handleImportEditorAsset, handleOpenWiki,
    openLinkTarget, updateWorkspaceSettings, createAndOpenPage,
  } = actions;

  const requestTreeRename = useCallback((resource: Resource) => {
    setTreeRenameRequest((previous) => ({
      path: resource.path,
      token: (previous?.token ?? 0) + 1,
    }));
  }, []);

  const treeActions = useTreeActionsController({
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
    renameResource: resourceController.renameResource,
    clearSelectionIf: resourceController.clearSelectionIf,
    removeTabs: navigationController.removeTabs,
    createAndOpenPage,
    requestTreeRename,
    handleOpenExternally,
  });
  const {
    handleTreeResourceContextMenu,
    handleTreeFolderContextMenu,
    handleTreeRename,
    handleMoveToFolder,
  } = treeActions;

  function beginSidebarResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = sidebarWidth;
    const onMove = (move: PointerEvent) => {
      setSidebarWidth(Math.max(210, Math.min(480, startWidth + move.clientX - startX)));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  async function commitTitle() {
    await resourceController.commitTitle(titleDraft);
    setEditingTitle(false);
  }

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void saveSidebarWidth(sidebarWidth).catch(() => {});
    }, 250);
    return () => window.clearTimeout(timer);
  }, [sidebarWidth]);

  useEffect(() => {
    if (!profileReady) return;
    if (
      serializeResourceTreeCollapseState(resourceTreeCollapsedByWorkspace) ===
      serializeResourceTreeCollapseState(profile.resourceTreeCollapsedByWorkspace ?? {})
    ) {
      return;
    }
    const timer = window.setTimeout(() => {
      void saveResourceTreeCollapsed(profile, resourceTreeCollapsedByWorkspace).catch(() => {});
    }, 250);
    return () => window.clearTimeout(timer);
  }, [
    profile,
    profileReady,
    profile.resourceTreeCollapsedByWorkspace,
    resourceTreeCollapsedByWorkspace,
  ]);

  const handleTreeCollapsedPathsChange = useCallback((paths: ReadonlySet<string>) => {
    const workspaceKey = snapshotRef.current?.id;
    if (!workspaceKey) return;
    setResourceTreeCollapsedByWorkspace((previous) =>
      updateCollapsedPathsForWorkspace(previous, workspaceKey, paths),
    );
  }, []);

  const treeCollapsedPaths = useMemo(
    () => collapsedPathsForWorkspace(resourceTreeCollapsedByWorkspace, snapshot?.id ?? null),
    [resourceTreeCollapsedByWorkspace, snapshot?.id],
  );

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listen<{ root: string; path: string }>("open-resource", (event) => {
      const open = async () => {
        let current = snapshotRef.current;
        if (!current || current.root !== event.payload.root) {
          current = await invoke<WorkspaceSnapshot>("open_workspace", { path: event.payload.root });
          await adoptWorkspace(current);
        }
        const resource = current.resources.find((entry) => entry.path === event.payload.path);
        if (resource) await handleSelect(resource);
      };
      void open().catch((err) => setError(String(err)));
    }).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten?.();
    // Listener uses refs / event payload and should only be installed once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (inBrowser) return;
    void register(QUICK_NOTE_SHORTCUT, () => {
      void showQuickNote(snapshotRef.current?.root).catch((err) => setError(String(err)));
    }).catch((err) => {
      console.warn("global Quick Note shortcut unavailable:", err);
    });
    return () => {
      void unregister(QUICK_NOTE_SHORTCUT);
    };
  }, []);

  const paletteItems = useMemo<PaletteItem[]>(() => {
    const root = snapshot?.root ?? null;
    const actions: PaletteItem[] = [
      { id: "action:new-page", label: "New page", run: handleNewPage },
      { id: "action:new-table", label: "New table…", run: () => void handleNewTable() },
      { id: "action:import-csv", label: "Import CSV…", run: () => void handleImportCsv() },
      { id: "action:quick-note", label: "Quick note", hint: "Cmd+N", run: handleQuickNote },
      { id: "action:new-workspace", label: "New workspace…", run: () => void openNewWorkspaceDialog() },
      { id: "action:open-workspace", label: "Open workspace…", run: () => void handleOpenWorkspace() },
      {
        id: "action:search",
        label: "Search workspace…",
        hint: "Cmd+K",
        run: () => setSearchPaneOpen(true),
      },
      {
        id: "action:theme-follow-system",
        label: "Theme: Follow system",
        hint:
          themeCatalog?.resolved.settings.mode === "auto"
            ? "active"
            : "auto dark/light pair",
        run: () => {
          void (async () => {
            try {
              applyThemeCatalog(await setAppearanceMode("auto", root));
              setStatusToast("Theme follows system");
            } catch (err) {
              setError(String(err));
            }
          })();
        },
      },
    ];

    const themes: ThemeSummaryPayload[] = themeCatalog?.themes ?? [];
    for (const theme of themes) {
      const active = themeCatalog?.resolved.id === theme.id;
      actions.push({
        id: `action:theme-${theme.id}`,
        label: `Theme: ${theme.name}`,
        hint: active
          ? "active"
          : theme.source === "user"
            ? "user"
            : theme.appearance,
        run: () => {
          void (async () => {
            try {
              applyThemeCatalog(await setFixedTheme(theme.id, root));
              setStatusToast(`Theme: ${theme.name}`);
            } catch (err) {
              setError(String(err));
            }
          })();
        },
      });
    }

    if (!inBrowser) {
      actions.push({ id: "action:undo", label: "Undo last change", run: () => void handleUndo() });
    }

    if (selected && session?.kind === "page") {
      actions.push({
        id: "action:insert-resource-link",
        label: `Insert link to ${selected.path}`,
        hint: "page",
        run: () => {
          setStatusToast(`Drag ${selected.path} onto the page, or paste [[${selected.path.split("/").pop()}]]`);
        },
      });
      actions.push({
        id: "action:insert-resource-embed",
        label: `Insert embed of ${selected.path}`,
        hint: "Alt-drop",
        run: () => {
          setStatusToast(`Alt-drop ${selected.path} onto the page to embed`);
        },
      });
    }

    if (selected && session?.kind === "canvas") {
      actions.push({
        id: "action:place-on-canvas",
        label: `Place ${selected.path} on canvas`,
        hint: "drop or toolbar",
        run: () => {
          setStatusToast(`Drop ${selected.path} onto the canvas, or use Place on Canvas`);
        },
      });
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
  }, [snapshot, selected, session, themeCatalog, applyThemeCatalog]);

  const handleQuickNoteRef = useRef(handleQuickNote);
  handleQuickNoteRef.current = handleQuickNote;
  const handleNewPageRef = useRef(handleNewPage);
  handleNewPageRef.current = handleNewPage;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (matchesKeybinding(event, settings.keybindings.search)) {
        event.preventDefault();
        setPaletteOpen(false);
        setSearchPaneOpen(true);
      } else if (matchesKeybinding(event, settings.keybindings.commandPalette)) {
        event.preventDefault();
        setSearchPaneOpen(false);
        setPaletteOpen(true);
      } else if (matchesKeybinding(event, settings.keybindings.quickNote)) {
        event.preventDefault();
        setPaletteOpen(false);
        handleQuickNoteRef.current();
      } else if (matchesKeybinding(event, settings.keybindings.newPage)) {
        event.preventDefault();
        handleNewPageRef.current();
      } else if (matchesKeybinding(event, settings.keybindings.settings)) {
        event.preventDefault();
        navigationController.setActivityArea("settings");
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [settings.keybindings]);

  return {
    profile, profileReady, settings, startup, snapshot, snapshotRef, selected, session, error, busy, saveState,
    externalConflict, reloadToken, newWorkspaceOpen, workspacesDir, templates, statusToast, runtimeNotice,
    profileNotices, paletteOpen, searchPaneOpen, themeCatalog, activityArea, sidebarWidth, treeCollapsedPaths, revealPath, linkPicker,
    linkRepairReview, handleLinkRepairAccept, handleLinkRepairDefer,
    openTabs, navigation, inspectorOpen, editingTitle, titleDraft, assetRoot, wikiTargets, pageEditorRef,
    recents, page, currentPageRevisionRef,
    paletteItems, hasCapability, setSettings, setStartup, setError,
    setSaveState, setNewWorkspaceOpen, setSearchPaneOpen, setPaletteOpen,
    setActivityArea, setInspectorOpen, setDismissedNoticeCodes, setEditingTitle, setTitleDraft, setSidebarWidth,
    handleTreeCollapsedPathsChange,
    setLinkPicker,
    setStatusToast, applyThemeCatalog, rememberWorkspace, clearRecents, resetSettings, handleGetStarted,
    handleOpenWorkspace, openRecent, handleCreateWorkspace, openNewWorkspaceDialog, pickWorkspaceFolder,
    handleNewPage, handleQuickNote, handleNewTable, handleImportCsv, handleUndo, handleSelect,
    handleOpenExternally, handleOpenFile, handleImportEditorAsset,
    navigateHistory: navigationController.navigateHistory,
    closeTab: navigationController.closeTab,
    reorderTab: navigationController.reorderTab,
    beginSidebarResize, commitTitle, updateWorkspaceSettings, handleOpenWiki, openLinkTarget,
    handleKeepIncoming, handleKeepLocal, handleKeepBoth,
    handleTreeResourceContextMenu, handleTreeFolderContextMenu, handleTreeRename, handleMoveToFolder,
    treeRenameRequest,
  };
}
