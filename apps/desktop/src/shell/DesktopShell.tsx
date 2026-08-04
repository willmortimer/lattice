import { BrandMark } from "../shell/BrandMark";
import { DictationControls } from "../shell/DictationControls";
import { voiceHintsFromPage } from "../lib/voice";
import { AllWorkspacesHome } from "../shell/AllWorkspacesHome";
import { ResourceInspector } from "../shell/ResourceInspector";
import { WorkspaceSwitcher } from "../shell/WorkspaceSwitcher";
import { ResourceSurface } from "../shell/ResourceSurface";
import { SaveStatusIndicator, TabUnsavedDot } from "../shell/SaveStatusIndicator";
import { StartupSplash } from "../shell/StartupSplash";
import { AppLockOverlay } from "../shell/AppLockOverlay";
import { DemoDriverHost } from "../demo/DemoDriverHost";
import { useStartupSplash } from "../shell/useStartupSplash";
import { useRendererServices } from "../shell/useRendererServices";
import { setAppearanceMode, setFixedTheme, setFontPack } from "../theme";
import { fileTitle } from "../controllers/useResourceController";
import { searchResourceLinks } from "../lib/resourceLinks";
import { Button, DialogBackdrop, DialogPopup, DialogPortal, DialogRoot, DialogTitle, IconButton, MenuItem, MenuPopup, MenuPortal, MenuPositioner, MenuRoot, MenuSeparator, MenuTrigger, TooltipProvider } from "@lattice/ui";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUpRight,
  CaretDown,
  DotsThree,
  FilePlus,
  Files,
  FolderPlus,
  Gear,
  House,
  List as MenuIcon,
  MagnifyingGlass,
  Plus,
  Robot,
  Sidebar,
  Sparkle,
  Table,
  Terminal,
  WarningCircle,
  X,
} from "@phosphor-icons/react";
import { listen } from "@tauri-apps/api/event";
import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import type { useDesktopController } from "../controllers/useDesktopController";
import { GuidanceTourController, requestShellTourStart, seedGuidanceAnchors } from "../guidance";
import { markShellTourFinished, shouldAutoStartShellTour } from "../guidance/tour/shellTourPersistence";
import { emitProductTelemetry } from "../lib/cloud";
import { inBrowser } from "../demo";
import { demoSearch } from "../demo";
import { TabularImportReviewDialog } from "../data/CsvImportReviewDialog";
import { LinkRepairReviewModal } from "../LinkRepairReviewModal";
import { ProposalApplyToast } from "../ProposalApplyToast";
import { ProposalInboxPanel } from "../ProposalInboxPanel";
import { ProposalReviewModal } from "../ProposalReviewModal";
import { batchWarnThresholdExceeded } from "../lib/linkRepair";
import { hasTauri } from "../lib/ipc";
import { readConnectedCheckoutFile } from "../ConnectedRoots";
import { GithubFileViewer } from "../GithubFileViewer";
import { NewWorkspaceDialog } from "../NewWorkspaceDialog";
import { useDesktopUiStore } from "./desktopUiStore";
import "./agentFocus.css";
import { useResourceTreeBadgeHints } from "./useResourceTreeBadgeHints";
import { ResourceTree } from "../ResourceTree";
import { KindMark } from "../KindMark";
import { QUICK_NOTE_SHORTCUT } from "../quickNoteWindow";
import { directoryPurposesFromCatalog } from "../lib/templates";
import { newFolderParentPath } from "../lib/treeOps";
import {
  applyAgentDetachedHandoffToSession,
  readAgentDetachedHandoff,
} from "../agent/agentDetachedHandoff";
import {
  AGENT_DETACHED_CLOSED_EVENT,
  requestCloseDetachedAgent,
  type AgentDetachedClosedPayload,
} from "../agent/agentDetachedWindow";
import { useAgentSessionStore } from "../agent/agentStore";

const SettingsPage = lazy(() =>
  import("../settings/SettingsPage").then((module) => ({ default: module.SettingsPage })),
);
const TerminalPanel = lazy(() =>
  import("../terminal/TerminalPanel").then((module) => ({ default: module.TerminalPanel })),
);
const SearchPane = lazy(() =>
  import("../SearchPane").then((module) => ({ default: module.SearchPane })),
);
const AgentPanelShell = lazy(() =>
  import("../agent/AgentPanelShell").then((module) => ({ default: module.AgentPanelShell })),
);
const LatticeAgentProvider = lazy(() =>
  import("../agent/LatticeAgentProvider").then((module) => ({
    default: module.LatticeAgentProvider,
  })),
);
const AgentPanelBody = lazy(() =>
  import("../agent/AgentPanelBody").then((module) => ({ default: module.AgentPanelBody })),
);
const AgentThread = lazy(() =>
  import("../agent/AgentThread").then((module) => ({ default: module.AgentThread })),
);
const AgentHeader = lazy(() =>
  import("../agent/AgentHeader").then((module) => ({ default: module.AgentHeader })),
);
const AgentOverlayHost = lazy(() =>
  import("../agent/AgentOverlayHost").then((module) => ({ default: module.AgentOverlayHost })),
);
const ConnectedRoots = lazy(() =>
  import("../ConnectedRoots").then((module) => ({ default: module.ConnectedRoots })),
);
const CommandPalette = lazy(() =>
  import("../CommandPalette").then((module) => ({ default: module.CommandPalette })),
);

export interface DesktopShellProps { model: ReturnType<typeof useDesktopController>; }

export function DesktopShell({ model }: DesktopShellProps) {
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [browserActiveFolderPath, setBrowserActiveFolderPath] = useState<string | null>(null);
  const {
    profile, profileReady, settings, startup, snapshot, catalog, catalogDelta, selected, selectedResourceIds, session, error, busy,
    externalConflict, reloadToken, newWorkspaceOpen, workspacesDir, templates, statusToast,
    setStatusToast,
    profileNotices, paletteOpen, searchPaneOpen, themeCatalog, activityArea, sidebarWidth,
    treeCollapsedPaths, revealPath, linkPicker, csvImportReview,
    settingsDeepLinkTarget, clearSettingsDeepLink,
    handleCancelCsvImport, handleConfirmCsvImport, handleCsvImportColumnTypeChange,
    linkRepairReview, handleLinkRepairAccept, handleLinkRepairDefer,
    proposalSummaries, proposalInboxLoading, proposalApplyOutcome, proposalReview, refreshProposalInbox, openProposalReview,
    handleProposalAccept, handleProposalReject, handleProposalCancel, handleCreateDemoProposal,
    openProposalResourcePath, dismissProposalApplyOutcome,
    openTabs, navigation, inspectorOpen, agentPanelOpen, editingTitle, titleDraft, assetRoot,
    wikiTargets, pageEditorRef, paletteItems, hasCapability, setSettings,
    applyDesktopSettings, applyStartupSettings, setError,
    recents, page, setLinkPicker, handleImportEditorAsset,
    setNewWorkspaceOpen, setSearchPaneOpen, setPaletteOpen, setActivityArea, setInspectorOpen, setAgentPanelOpen,
    setDismissedNoticeCodes, setEditingTitle, setTitleDraft, applyThemeCatalog,
    clearRecents, resetSettings, refreshProfile, handleGetStarted, handleOpenWorkspace, openRecent,
    openWorkspaceById, handleCreateWorkspace, openNewWorkspaceDialog, pickWorkspaceFolder, handleNewPage, handleQuickNote,
    handleNewTable, handleImportCsv, handlePromoteWorkspaceCsv, handleSelect, applyTreeSelection, handleOpenExternally, handleOpenFile,
    handleKeepIncoming, handleKeepLocal, handleKeepBoth, handleTreeCollapsedPathsChange,
    handleTreeResourceContextMenu, handleTreeFolderContextMenu, handleTreeRename, handleMoveToFolder,
    handleNewFolderInFolder,
    treeRenameRequest,
    navigateHistory, closeTab, reorderTab, beginSidebarResize, commitTitle, updateWorkspaceSettings,
    applyWorkspaceSettings, handleOpenWiki, openLinkTarget,     handleNotebookContentChange, handleRevisionChange,
    handlePagePersistModeChange,
    reloadPageFromDisk,
    setSession,
    appLock, setAppLock,
  } = model;

  const agentLayoutMode = useDesktopUiStore((state) => state.agentLayoutMode);
  const setAgentLayoutMode = useDesktopUiStore((state) => state.setAgentLayoutMode);
  const exitAgentFocus = useDesktopUiStore((state) => state.exitAgentFocus);
  const selectThreadId = useAgentSessionStore((state) => state.selectThreadId);
  const agentFocusActive = agentPanelOpen && agentLayoutMode === "focus";
  const reviewInWorkbench =
    Boolean(proposalReview) &&
    agentPanelOpen &&
    (agentLayoutMode === "workbench" || agentLayoutMode === "focus");
  const resourceTreeBadgeHints = useResourceTreeBadgeHints(
    proposalSummaries,
    agentPanelOpen,
    selected?.path,
  );

  useEffect(() => {
    void emitProductTelemetry("app_launch");
  }, []);

  useEffect(() => seedGuidanceAnchors(), []);

  useEffect(() => {
    if (!agentFocusActive) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) {
        return;
      }
      if (paletteOpen || searchPaneOpen) {
        return;
      }
      event.preventDefault();
      exitAgentFocus();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [agentFocusActive, exitAgentFocus, paletteOpen, searchPaneOpen]);

  useEffect(() => {
    if (!hasTauri) {
      return;
    }
    let cancelled = false;
    const unlistenPromise = listen<AgentDetachedClosedPayload>(
      AGENT_DETACHED_CLOSED_EVENT,
      (event) => {
        if (cancelled) {
          return;
        }
        const handoff = readAgentDetachedHandoff();
        if (handoff) {
          applyAgentDetachedHandoffToSession(handoff);
          selectThreadId(handoff.workspaceRoot, handoff.threadId);
        } else {
          selectThreadId(event.payload.workspaceRoot, event.payload.threadId);
        }
        setAgentLayoutMode(event.payload.returnLayoutMode);
        setAgentPanelOpen(true);
      },
    );
    return () => {
      cancelled = true;
      void unlistenPromise.then((stop) => stop());
    };
  }, [selectThreadId, setAgentLayoutMode, setAgentPanelOpen]);

  const sessionLocked = Boolean(appLock.enabled && appLock.locked);

  const splashVisible = useStartupSplash({
    enabled: startup.showStartupSplash !== false,
    profileReady,
    themeReady: themeCatalog !== null,
  });

  const shellTourAutoStarted = useRef(false);
  useEffect(() => {
    if (shellTourAutoStarted.current) return;
    if (
      !shouldAutoStartShellTour({
        profileReady,
        splashVisible,
        workspaceLoaded: snapshot !== null,
        settings,
      })
    ) {
      return;
    }
    shellTourAutoStarted.current = true;
    requestShellTourStart();
  }, [profileReady, settings, snapshot, splashVisible]);

  // Manifest-authored purposes (editable in lattice.yaml) win over the
  // catalog hints derived from the provisioning template.
  const directoryPurposes = useMemo(
    () => ({
      ...directoryPurposesFromCatalog(snapshot?.sourceTemplate),
      ...(snapshot?.directoryPurposes ?? {}),
    }),
    [snapshot?.sourceTemplate, snapshot?.directoryPurposes],
  );

  const browserNewFolderParent = useMemo(
    () => newFolderParentPath(selected, { activeFolderPath: browserActiveFolderPath }),
    [browserActiveFolderPath, selected],
  );

  const activeRendererSessionId = selected?.path ?? null;

  const rendererContext = useRendererServices({
    sessionId: activeRendererSessionId,
    assetRoot,
    workspaceRoot: inBrowser || !snapshot ? null : snapshot.root,
    resources: snapshot?.resources ?? [],
    catalog,
    settings,
    pageEditorRef,
    wikiTargets,
    conflict: externalConflict,
    reloadToken,
    handlers: {
      onRevisionChange: handleRevisionChange,
      onNotebookContentChange: handleNotebookContentChange,
      onOpenWiki: handleOpenWiki,
      onCreateTable: handleNewTable,
      onSearchWiki: !inBrowser && snapshot
        ? (query) => searchResourceLinks(snapshot.root, query, 20)
        : undefined,
      onImportAsset: inBrowser ? undefined : handleImportEditorAsset,
      onKeepIncoming: () => void handleKeepIncoming(),
      onKeepLocal: () => void handleKeepLocal(),
      onKeepBoth: () => void handleKeepBoth(),
      onOpenFile: handleOpenFile,
      onOpenProposal: (proposalId) => void openProposalReview(proposalId),
      onOpenExternally: inBrowser
        ? undefined
        : (resource) => void handleOpenExternally(resource),
      onPromoteWorkspaceCsv: inBrowser
        ? undefined
        : (resource) => void handlePromoteWorkspaceCsv(resource),
      onPageWidthChange: (pageWidth) =>
        setSettings((current) => ({
          ...current,
          editor: { ...current.editor, pageWidth },
        })),
      onPersistModeChange: handlePagePersistModeChange,
      openInspectorOnWiki: settings.editor.linkClickBehavior === "inspect",
    },
  });

  if (splashVisible) {
    return (
      <>
        <div className="native-titlebar" data-tauri-drag-region />
        <StartupSplash />
      </>
    );
  }

  if (sessionLocked) {
    return (
      <>
        <div className="native-titlebar" data-tauri-drag-region />
        <AppLockOverlay
          onUnlocked={() => setAppLock((current) => ({ ...current, locked: false }))}
        />
      </>
    );
  }

  if (!snapshot) {
    return (
      <>
        <div className="native-titlebar" data-tauri-drag-region />
        <div className="empty-state">
          <BrandMark />
          <h1 className="empty-wordmark">Lattice</h1>
          <p className="empty-copy">
            Create a Personal workspace, choose a template from the gallery, or
            open a folder that already contains <code>lattice.yaml</code>. Lattice
            never restores externally deleted workspace content automatically.
          </p>
          {profileNotices.map((notice) => (
            <div className="profile-notice profile-notice-empty" role="status" key={notice.code}>
              <WarningCircle size={16} />
              <div>
                <strong>{notice.title}</strong>
                <span>{notice.message}</span>
                {notice.path && <code>{notice.path}</code>}
              </div>
            </div>
          ))}
          <div className="empty-actions">
            <button className="primary-button" onClick={() => void handleGetStarted()} disabled={busy}>
              {busy ? "Setting up…" : "Create Lattice home"}
            </button>
            <button
              className="secondary-button"
              onClick={() => void openNewWorkspaceDialog()}
              disabled={busy || !profileReady}
            >
              New workspace in a folder…
            </button>
            <button className="secondary-button" onClick={() => void handleOpenWorkspace()} disabled={busy}>
              Open existing workspace…
            </button>
          </div>
          {recents.length > 0 && (
            <div className="recent-workspaces">
              <div className="recent-heading">Recent</div>
              {recents.slice(0, 5).map((r) => (
                <button
                  key={r.root}
                  type="button"
                  className="recent-item"
                  onClick={() => void openRecent(r.root)}
                  disabled={busy}
                  title={r.root}
                >
                  <span className="recent-title">{r.title}</span>
                  <code className="recent-path">{r.root}</code>
                </button>
              ))}
            </div>
          )}
          <code className="empty-hint">Your default workspace can be changed when creating another workspace.</code>
          {error && <p className="error-text">{error}</p>}
        </div>
        <NewWorkspaceDialog
          open={newWorkspaceOpen}
          busy={busy}
          templates={templates}
          workspacesDir={workspacesDir ?? profile.workspacesDirectory}
          hasValidDefault={profile.hasValidConfiguredDefault}
          onCancel={() => setNewWorkspaceOpen(false)}
          onPickFolder={pickWorkspaceFolder}
          onCreate={(args) => void handleCreateWorkspace(args)}
        />
      </>
    );
  }

  return (
    <TooltipProvider>
      <div className={agentFocusActive ? "shell shell-agent-focus" : "shell"}>
        <div className="native-titlebar" data-tauri-drag-region />
        <aside className="activity-rail" aria-label="Workspace areas">
          <div className="activity-brand">
            <BrandMark size={28} />
          </div>
          <nav>
            {[
              { id: "home" as const, label: "Home", icon: House },
              { id: "files" as const, label: "Files", icon: Files },
              { id: "search" as const, label: "Search", icon: MagnifyingGlass },
              { id: "quick-note" as const, label: "Quick Capture", icon: Sparkle },
            ].map(({ id, label, icon: Icon }) => (
              <IconButton
                key={id}
                label={label}
                className={activityArea === id ? "activity-button-active" : ""}
                onClick={() => {
                  if (id === "search") {
                    setActivityArea("search");
                    setSearchPaneOpen(true);
                  } else if (id === "quick-note") {
                    setActivityArea("quick-note");
                    handleQuickNote();
                  } else {
                    setActivityArea(id);
                  }
                }}
              >
                <Icon size={17} />
              </IconButton>
            ))}
          </nav>
          {hasCapability("terminal") && (
            <IconButton
              label="Terminal"
              className={terminalOpen ? "activity-button-active" : ""}
              onClick={() => setTerminalOpen((open) => !open)}
            >
              <Terminal size={17} />
            </IconButton>
          )}
          <IconButton
            label={agentPanelOpen ? "Hide agent" : "Show agent"}
            className={agentPanelOpen ? "activity-button-active" : ""}
            data-guidance-anchor="agent.panel.toggle"
            onClick={() => setAgentPanelOpen((open) => !open)}
          >
            <Robot size={17} />
          </IconButton>
          <div className="activity-spacer" />
          <IconButton
            label="Settings"
            className={activityArea === "settings" ? "activity-button-active" : ""}
            onClick={() => setActivityArea("settings")}
          >
            <Gear size={17} />
          </IconButton>
        </aside>

        <aside className="sidebar" style={{ width: sidebarWidth }}>
          <header className="sidebar-head">
            <div className="workspace-title-row">
              <div className="workspace-title" title={snapshot.root}>
                <WorkspaceSwitcher
                  title={snapshot.title}
                  activeWorkspaceId={snapshot.id}
                  pinnedRoot={profile.effectiveDefaultWorkspace}
                  recents={recents}
                  busy={busy}
                  markGuidanceAnchor
                  onOpenById={(workspaceId) => void openWorkspaceById(workspaceId)}
                  onCreate={() => void openNewWorkspaceDialog()}
                  onOpenFolder={() => void handleOpenWorkspace()}
                  onOpenInNewWindow={() => {
                    setStatusToast("Open in new window is not available yet.");
                  }}
                  onManage={() => setActivityArea("home")}
                />
              </div>
              <IconButton label="Workspace menu" onClick={() => setPaletteOpen(true)}>
                <DotsThree size={15} />
              </IconButton>
            </div>
            <div className="workspace-root">{`⁦${snapshot.root}⁩`}</div>
          </header>
          <div className="sidebar-toolbar">
            <Button
              variant="ghost"
              size="sm"
              className="sidebar-search"
              data-guidance-anchor="shell.search"
              onClick={() => setSearchPaneOpen(true)}
            >
              <MagnifyingGlass size={14} />
              Search
              <kbd>{settings.keybindings.search}</kbd>
            </Button>
            <MenuRoot>
              <MenuTrigger
                render={
                  <IconButton label="Create resource" data-guidance-anchor="resource-tree.new-page">
                    <Plus size={15} />
                  </IconButton>
                }
              />
              <MenuPortal>
                <MenuPositioner sideOffset={6} align="end">
                  <MenuPopup className="ltui-menu">
                    <MenuItem className="ltui-menu-item" onClick={handleNewPage}>
                      <FilePlus size={14} />
                      New page
                    </MenuItem>
                    {hasCapability("sqlite") && (
                      <MenuItem className="ltui-menu-item" onClick={() => void handleNewTable()}>
                        <Table size={14} />
                        New table
                      </MenuItem>
                    )}
                    <MenuSeparator className="ltui-menu-separator" />
                    {hasCapability("sqlite") && (
                      <MenuItem className="ltui-menu-item" onClick={() => void handleImportCsv()}>
                        <ArrowUpRight size={14} />
                        Import table
                      </MenuItem>
                    )}
                  </MenuPopup>
                </MenuPositioner>
              </MenuPortal>
            </MenuRoot>
            <IconButton
              label={`New folder in ${browserNewFolderParent || "workspace root"}`}
              onClick={() => void handleNewFolderInFolder(browserNewFolderParent)}
            >
              <FolderPlus size={15} />
            </IconButton>
          </div>
          <nav className="resource-list">
            <ResourceTree
              catalog={catalog}
              catalogDelta={catalogDelta}
              selectedResourceIds={selectedResourceIds}
              onTreeSelect={applyTreeSelection}
              directoryPurposes={directoryPurposes}
              workspaceKey={snapshot.id}
              collapsedPaths={treeCollapsedPaths}
              onCollapsedPathsChange={handleTreeCollapsedPathsChange}
              onResourceContextMenu={handleTreeResourceContextMenu}
              onFolderContextMenu={handleTreeFolderContextMenu}
              onRename={handleTreeRename}
              onMoveToFolder={(fromPaths, toDir) => void handleMoveToFolder(fromPaths, toDir)}
              renameRequest={treeRenameRequest}
              revealPath={revealPath}
              badgeHints={resourceTreeBadgeHints}
              activeFolderPath={inBrowser ? browserActiveFolderPath : null}
              onActiveFolderChange={inBrowser ? setBrowserActiveFolderPath : undefined}
            />
            {!inBrowser && (
              <Suspense fallback={null}>
                <ConnectedRoots
                  workspaceRoot={snapshot.root}
                  onError={setError}
                  onOpenFile={(detail) => {
                    void readConnectedCheckoutFile(
                      detail.provider,
                      snapshot.root,
                      detail.bindingId,
                      detail.path,
                    )
                      .then((file) => {
                        const scheme = detail.provider === "github" ? "github" : "gitlab";
                        setSession({
                          kind: "github-file",
                          bindingId: detail.bindingId,
                          owner: detail.owner,
                          repo: detail.repo,
                          path: detail.path,
                          content: file.content,
                          stale: detail.stale,
                          resource: {
                            path: `${scheme}://${detail.owner}/${detail.repo}/${detail.path}`,
                            kind: "file",
                          },
                        });
                        setActivityArea("files");
                      })
                      .catch((error) =>
                        setError(error instanceof Error ? error.message : String(error)),
                      );
                  }}
                />
              </Suspense>
            )}
          </nav>
          {hasTauri && (
            <ProposalInboxPanel
              proposals={proposalSummaries}
              busy={busy}
              loading={proposalInboxLoading}
              onRefresh={refreshProposalInbox}
              onOpen={(proposalId) => void openProposalReview(proposalId)}
              onCreateDemo={() => void handleCreateDemoProposal()}
            />
          )}
          <div className="sidebar-footer">
            <Button variant="ghost" size="sm" onClick={() => void openNewWorkspaceDialog()}>
              New workspace…
            </Button>
            <Button variant="ghost" size="sm" onClick={() => void handleOpenWorkspace()}>
              Open workspace…
            </Button>
          </div>
          <div
            className="sidebar-resize"
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize resource sidebar"
            onPointerDown={beginSidebarResize}
          />
        </aside>

        <main className="main-pane">
          <header className="main-head">
            <div className="nav-controls">
              <IconButton
                label="Back"
                disabled={navigation.index <= 0}
                onClick={() => navigateHistory(-1)}
              >
                <ArrowLeft size={15} />
              </IconButton>
              <IconButton
                label="Forward"
                disabled={navigation.index >= navigation.paths.length - 1}
                onClick={() => navigateHistory(1)}
              >
                <ArrowRight size={15} />
              </IconButton>
            </div>
            <div className="breadcrumbs">
              <WorkspaceSwitcher
                title={snapshot.title}
                activeWorkspaceId={snapshot.id}
                pinnedRoot={profile.effectiveDefaultWorkspace}
                recents={recents}
                busy={busy}
                onOpenById={(workspaceId) => void openWorkspaceById(workspaceId)}
                onCreate={() => void openNewWorkspaceDialog()}
                onOpenFolder={() => void handleOpenWorkspace()}
                onOpenInNewWindow={() => {
                  setStatusToast("Open in new window is not available yet.");
                }}
                onManage={() => setActivityArea("home")}
              />
              {selected?.path.split("/").slice(0, -1).map((part, index) => (
                <span key={`${part}:${index}`}>
                  <CaretDown size={11} />
                  {part}
                </span>
              ))}
              {selected && (
                <>
                  <CaretDown size={11} />
                  <KindMark kind={selected.kind} size={13} />
                  {editingTitle ? (
                    <input
                      className="title-input"
                      value={titleDraft}
                      autoFocus
                      onChange={(event) => setTitleDraft(event.target.value)}
                      onBlur={() => void commitTitle()}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") void commitTitle();
                        if (event.key === "Escape") {
                          setEditingTitle(false);
                          setTitleDraft(fileTitle(selected.path));
                        }
                      }}
                    />
                  ) : (
                    <button
                      type="button"
                      className="resource-title-button"
                      onDoubleClick={() => setEditingTitle(true)}
                      title="Double-click to rename"
                    >
                      {fileTitle(selected.path)}
                    </button>
                  )}
                </>
              )}
            </div>
            <div className="header-actions">
              {selected?.kind === "page" && page && (
                <SaveStatusIndicator
                  sessionId={activeRendererSessionId}
                  externalConflict={Boolean(externalConflict)}
                />
              )}
              {selected?.kind === "page" && page && !inBrowser && (
                <DictationControls
                  enabled
                  documentKey={
                    selected?.kind === "page" && page
                      ? `${selected.path}#${reloadToken}`
                      : null
                  }
                  voiceContext={voiceHintsFromPage({
                    documentPath: selected.path,
                    pageTitle: fileTitle(selected.path),
                    workspaceName: snapshot?.title ?? null,
                    rawContent: page.content,
                  })}
                  pageEditorRef={pageEditorRef}
                  onError={(message) => setError(message)}
                />
              )}
              {selected && !inBrowser && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => void handleOpenExternally(selected)}
                >
                  <ArrowUpRight size={13} />
                  Open
                </Button>
              )}
              <IconButton
                label={agentPanelOpen ? "Hide agent" : "Show agent"}
                className={agentPanelOpen ? "header-button-active" : ""}
                onClick={() => setAgentPanelOpen((open) => !open)}
              >
                <Robot size={16} />
              </IconButton>
              <IconButton
                label={inspectorOpen ? "Hide inspector" : "Show inspector"}
                className={inspectorOpen ? "header-button-active" : ""}
                onClick={() => setInspectorOpen((open) => !open)}
              >
                <Sidebar size={16} />
              </IconButton>
              <IconButton label="Command palette" onClick={() => setPaletteOpen(true)}>
                <MenuIcon size={16} />
              </IconButton>
            </div>
          </header>

          {profileNotices[0] && (
            <div className="profile-notice profile-notice-shell" role="status">
              <WarningCircle size={15} />
              <div>
                <strong>{profileNotices[0].title}</strong>
                <span>{profileNotices[0].message}</span>
              </div>
              <IconButton
                label="Dismiss notice"
                onClick={() =>
                  setDismissedNoticeCodes((codes) => [...codes, profileNotices[0].code])
                }
              >
                <X size={13} />
              </IconButton>
            </div>
          )}

          {openTabs.length > 0 && (
            <div className="tab-strip" role="tablist" aria-label="Open resources">
              {openTabs.map((tab) => (
                <button
                  type="button"
                  role="tab"
                  aria-selected={selected?.path === tab.path && activityArea === "files"}
                  draggable
                  className={selected?.path === tab.path ? "resource-tab resource-tab-active" : "resource-tab"}
                  key={tab.path}
                  onClick={() => void handleSelect(tab)}
                  onDragStart={(event) => event.dataTransfer.setData("text/lattice-tab", tab.path)}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) =>
                    reorderTab(event.dataTransfer.getData("text/lattice-tab"), tab.path)
                  }
                >
                  <KindMark kind={tab.kind} size={12} />
                  <span>{fileTitle(tab.path)}</span>
                  <TabUnsavedDot path={tab.path} />
                  <span
                    className="tab-close"
                    role="button"
                    tabIndex={0}
                    aria-label={`Close ${fileTitle(tab.path)}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      closeTab(tab.path);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") closeTab(tab.path);
                    }}
                  >
                    <X size={12} />
                  </span>
                </button>
              ))}
            </div>
          )}

          <div className="workspace-content">
            <Suspense fallback={null}>
              <AgentOverlayHost />
            </Suspense>
            <section className="content-pane">
              {activityArea === "home" && (
                <AllWorkspacesHome
                  activeWorkspaceId={snapshot.id}
                  activeWorkspaceTitle={snapshot.title}
                  pinnedRoot={profile.effectiveDefaultWorkspace}
                  recents={recents}
                  busy={busy}
                  onOpenById={(workspaceId) => void openWorkspaceById(workspaceId)}
                  onCreate={() => void openNewWorkspaceDialog()}
                  onOpenFolder={() => void handleOpenWorkspace()}
                  onImport={() => {
                    setStatusToast("Workspace import is not available yet.");
                  }}
                />
              )}

              {activityArea === "settings" && (
                <Suspense fallback={<div className="surface-loading">Loading settings…</div>}>
                  <SettingsPage
                    settings={settings}
                    startup={startup}
                    workspace={snapshot}
                    themeCatalog={themeCatalog}
                    onChange={setSettings}
                    onApplySettings={applyDesktopSettings}
                    onApplyStartup={applyStartupSettings}
                    onWorkspaceChange={(next) => void updateWorkspaceSettings(next)}
                    onApplyWorkspaceChange={applyWorkspaceSettings}
                    onClearRecents={clearRecents}
                    onReset={resetSettings}
                    onRefreshProfile={refreshProfile}
                    onThemeChange={(themeId) =>
                      void setFixedTheme(themeId, snapshot.root)
                        .then(applyThemeCatalog)
                        .catch((err) => setError(String(err)))
                    }
                    onFollowSystem={() =>
                      void setAppearanceMode("auto", snapshot.root)
                        .then(applyThemeCatalog)
                        .catch((err) => setError(String(err)))
                    }
                    onFontPackChange={(fontPack) =>
                      void setFontPack(fontPack, snapshot.root)
                        .then(applyThemeCatalog)
                        .catch((err) => setError(String(err)))
                    }
                    deepLinkTarget={settingsDeepLinkTarget}
                    onDeepLinkConsumed={clearSettingsDeepLink}
                  />
                </Suspense>
              )}

              {activityArea !== "home" && activityArea !== "settings" && (
                session?.kind === "canvas" ? (
                  <div className="canvas-pane">
                    <ResourceSurface
                      session={session}
                      capabilities={snapshot.capabilities}
                      context={rendererContext}
                    />
                  </div>
                ) : (
                  <div className="main-scroll">
                    {session?.kind === "github-file" ? (
                      <GithubFileViewer session={session} />
                    ) : (
                      <>
                    {!selected && !session && (
                      <div className="placeholder">
                        <p className="placeholder-copy">Select a resource from Files.</p>
                        <p className="placeholder-sub">⌘N opens Quick Note · {QUICK_NOTE_SHORTCUT} works globally</p>
                      </div>
                    )}
                    {session && (
                      <ResourceSurface
                        session={session}
                        capabilities={snapshot.capabilities}
                        context={rendererContext}
                      />
                    )}
                      </>
                    )}
                  </div>
                )
              )}

              {error && (
                <div className="bottom-panel" role="alert">
                  <WarningCircle size={15} />
                  <div>
                    <strong>Problem</strong>
                    <span>{error}</span>
                  </div>
                  <IconButton label="Dismiss problem" onClick={() => setError(null)}>
                    <X size={14} />
                  </IconButton>
                </div>
              )}
              {!error && busy && (
                <div className="bottom-panel bottom-panel-job" aria-live="polite">
                  <span className="job-spinner" />
                  <div>
                    <strong>Working</strong>
                    <span>Loading or applying a bounded workspace operation…</span>
                  </div>
                </div>
              )}
            </section>

            {inspectorOpen && (
              <ResourceInspector
                root={assetRoot}
                resource={selected}
                pageContent={session?.kind === "page" ? session.content : null}
                dataSnapshot={session?.kind === "data-app" || session?.kind === "interface" ? session.snapshot : null}
                error={error}
                onClose={() => setInspectorOpen(false)}
                onOpenFile={handleOpenFile}
                onReloadActivePage={() => {
                  void reloadPageFromDisk();
                }}
              />
            )}

            {agentPanelOpen && (
              <Suspense fallback={<div className="surface-loading">Loading agent…</div>}>
                <AgentPanelShell>
                  {agentLayoutMode === "detached" ? (
                    <>
                      <AgentHeader
                        onClose={() => {
                          void requestCloseDetachedAgent();
                          setAgentPanelOpen(false);
                        }}
                        workspaceRoot={inBrowser ? null : snapshot.root}
                      />
                      <AgentPanelBody
                        thread={null}
                        proposals={hasTauri ? proposalSummaries : []}
                        proposalLoading={hasTauri ? proposalInboxLoading : false}
                        onOpenProposal={hasTauri ? openProposalReview : undefined}
                      />
                    </>
                  ) : (
                    <LatticeAgentProvider workspaceRoot={inBrowser ? null : snapshot.root}>
                      <AgentHeader
                        onClose={() => setAgentPanelOpen(false)}
                        workspaceRoot={inBrowser ? null : snapshot.root}
                      />
                      <AgentPanelBody
                        thread={
                          <AgentThread
                            workspaceRoot={inBrowser ? null : snapshot.root}
                            activeResourcePath={selected?.path ?? null}
                            onNotify={setStatusToast}
                          />
                        }
                        proposals={hasTauri ? proposalSummaries : []}
                        proposalLoading={hasTauri ? proposalInboxLoading : false}
                        onOpenProposal={hasTauri ? openProposalReview : undefined}
                        proposalReview={reviewInWorkbench ? proposalReview : null}
                        proposalReviewBusy={busy}
                        workspaceRoot={inBrowser ? null : snapshot.root}
                        onProposalAccept={(selectedCommandIndices) =>
                          void handleProposalAccept(selectedCommandIndices)
                        }
                        onProposalReject={() => void handleProposalReject()}
                        onProposalCancel={handleProposalCancel}
                      />
                    </LatticeAgentProvider>
                  )}
                </AgentPanelShell>
              </Suspense>
            )}
          </div>

          {terminalOpen && (
            <Suspense fallback={null}>
              <TerminalPanel
                workspaceRoot={inBrowser ? null : snapshot.root}
                hasTerminalCapability={hasCapability("terminal")}
                onClose={() => setTerminalOpen(false)}
              />
            </Suspense>
          )}
        </main>

      {paletteOpen && (
        <Suspense fallback={null}>
          <CommandPalette items={paletteItems} onClose={() => setPaletteOpen(false)} />
        </Suspense>
      )}
      {searchPaneOpen && (
        <Suspense fallback={null}>
          <SearchPane
            root={assetRoot}
            semanticEnabled={settings.search.semanticEnabled}
            demoSearch={inBrowser ? demoSearch : () => []}
            onOpenFile={(path) => {
              setSearchPaneOpen(false);
              handleOpenFile(path);
            }}
            onClose={() => setSearchPaneOpen(false)}
          />
        </Suspense>
      )}
      {linkRepairReview && (
        <LinkRepairReviewModal
          plan={linkRepairReview.plan}
          mode={linkRepairReview.mode}
          moves={linkRepairReview.moves}
          busy={busy}
          truncated={linkRepairReview.batchPlan?.truncated ?? false}
          omittedCoMovedCount={linkRepairReview.batchPlan?.omittedCoMovedCount ?? 0}
          warnLargeRepairSet={
            linkRepairReview.batchPlan
              ? batchWarnThresholdExceeded(linkRepairReview.batchPlan)
              : false
          }
          onAccept={(acceptedCandidateIds) => void handleLinkRepairAccept(acceptedCandidateIds)}
          onDefer={() => void handleLinkRepairDefer()}
        />
      )}
      {proposalReview && !reviewInWorkbench && (
        <ProposalReviewModal
          proposal={proposalReview}
          workspaceRoot={snapshot.root}
          busy={busy}
          onAccept={(selectedCommandIndices) => void handleProposalAccept(selectedCommandIndices)}
          onReject={() => void handleProposalReject()}
          onCancel={handleProposalCancel}
        />
      )}
      {linkPicker && (
        <DialogRoot open onOpenChange={(open) => !open && setLinkPicker(null)}>
          <DialogPortal>
            <DialogBackdrop className="modal-backdrop" />
            <DialogPopup className="modal-panel link-picker-panel">
            <DialogTitle id="link-picker-title">Choose “{linkPicker.query}”</DialogTitle>
            <p className="modal-copy">More than one resource matches this link.</p>
            <div className="link-picker-list">
              {linkPicker.candidates.map((candidate) => (
                <button
                  type="button"
                  key={candidate.path}
                  onClick={() => {
                    openLinkTarget(candidate);
                    setLinkPicker(null);
                  }}
                >
                  <KindMark kind={candidate.kind} size={14} />
                  <span>
                    <strong>{candidate.display}</strong>
                    <small>{candidate.path}</small>
                  </span>
                </button>
              ))}
            </div>
            <div className="modal-actions">
              <Button onClick={() => setLinkPicker(null)}>Cancel</Button>
            </div>
            </DialogPopup>
          </DialogPortal>
        </DialogRoot>
      )}
      {csvImportReview && (
        <TabularImportReviewDialog
          review={csvImportReview}
          busy={busy}
          onCancel={handleCancelCsvImport}
          onConfirm={() => void handleConfirmCsvImport()}
          onColumnTypeChange={handleCsvImportColumnTypeChange}
        />
      )}
      <NewWorkspaceDialog
        open={newWorkspaceOpen}
        busy={busy}
        templates={templates}
        workspacesDir={workspacesDir ?? profile.workspacesDirectory}
        hasValidDefault={profile.hasValidConfiguredDefault}
        onCancel={() => setNewWorkspaceOpen(false)}
        onPickFolder={pickWorkspaceFolder}
        onCreate={(args) => void handleCreateWorkspace(args)}
      />
      {proposalApplyOutcome && (
        <ProposalApplyToast
          transactionId={proposalApplyOutcome.transactionId}
          openPaths={proposalApplyOutcome.openPaths}
          onOpenPath={(path) => void openProposalResourcePath(path)}
          onDismiss={dismissProposalApplyOutcome}
        />
      )}
      {statusToast && <div className="status-toast">{statusToast}</div>}
      <GuidanceTourController
        onShellTourFinished={() => {
          setSettings((current) => markShellTourFinished(current));
        }}
      />
      <DemoDriverHost />
    </div>
    </TooltipProvider>
  );
}
