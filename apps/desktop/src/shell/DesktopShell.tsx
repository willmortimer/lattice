import { inBrowser } from "../demo";
import { demoSearch } from "../demo";
import { TabularImportReviewDialog } from "../data/CsvImportReviewDialog";
import { LinkRepairReviewModal } from "../LinkRepairReviewModal";
import { batchWarnThresholdExceeded } from "../lib/linkRepair";
import { NewWorkspaceDialog } from "../NewWorkspaceDialog";
import { CommandPalette } from "../CommandPalette";
import { ResourceTree } from "../ResourceTree";
import { SearchPane } from "../SearchPane";
import { KindMark } from "../KindMark";
import { QUICK_NOTE_SHORTCUT } from "../quickNoteWindow";
import { directoryPurposesFromCatalog } from "../lib/templates";
import { newFolderParentPath } from "../lib/treeOps";
import { SettingsPage } from "../settings/SettingsPage";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { BrandMark } from "../shell/BrandMark";
import { DictationControls } from "../shell/DictationControls";
import { voiceHintsFromPage } from "../lib/voice";
import { HomeDashboard } from "../shell/HomeDashboard";
import { ResourceInspector } from "../shell/ResourceInspector";
import { ResourceSurface } from "../shell/ResourceSurface";
import { StartupSplash } from "../shell/StartupSplash";
import { useStartupSplash } from "../shell/useStartupSplash";
import { setAppearanceMode, setFixedTheme } from "../theme";
import { fileTitle } from "../controllers/useResourceController";
import { isUnsaved, saveIndicatorText } from "../editor/saveState";
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
  Sidebar,
  Sparkle,
  Table,
  Terminal,
  WarningCircle,
  X,
} from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import type { useDesktopController } from "../controllers/useDesktopController";

export interface DesktopShellProps { model: ReturnType<typeof useDesktopController>; }

export function DesktopShell({ model }: DesktopShellProps) {
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [browserActiveFolderPath, setBrowserActiveFolderPath] = useState<string | null>(null);
  const {
    profile, profileReady, settings, startup, snapshot, selected, selectedPaths, session, error, busy, saveState,
    externalConflict, reloadToken, newWorkspaceOpen, workspacesDir, templates, statusToast,
    profileNotices, paletteOpen, searchPaneOpen, themeCatalog, activityArea, sidebarWidth,
    treeCollapsedPaths, revealPath, linkPicker, csvImportReview,
    handleCancelCsvImport, handleConfirmCsvImport, handleCsvImportColumnTypeChange,
    linkRepairReview, handleLinkRepairAccept, handleLinkRepairDefer,
    openTabs, navigation, inspectorOpen, editingTitle, titleDraft, assetRoot,
    wikiTargets, pageEditorRef, paletteItems, hasCapability, setSettings, setStartup, setError,
    recents, page, setSaveState, setLinkPicker, handleImportEditorAsset,
    setNewWorkspaceOpen, setSearchPaneOpen, setPaletteOpen, setActivityArea, setInspectorOpen,
    setDismissedNoticeCodes, setEditingTitle, setTitleDraft, applyThemeCatalog,
    clearRecents, resetSettings, handleGetStarted, handleOpenWorkspace, openRecent,
    handleCreateWorkspace, openNewWorkspaceDialog, pickWorkspaceFolder, handleNewPage, handleQuickNote,
    handleNewTable, handleImportCsv, handlePromoteWorkspaceCsv, handleSelect, applyTreeSelection, handleOpenExternally, handleOpenFile,
    handleKeepIncoming, handleKeepLocal, handleKeepBoth, handleTreeCollapsedPathsChange,
    handleTreeResourceContextMenu, handleTreeFolderContextMenu, handleTreeRename, handleMoveToFolder,
    handleNewFolderInFolder,
    treeRenameRequest,
    navigateHistory, closeTab, reorderTab, beginSidebarResize, commitTitle, updateWorkspaceSettings,
    handleOpenWiki, openLinkTarget, handleNotebookContentChange, handleRevisionChange,
  } = model;

  const splashVisible = useStartupSplash({
    enabled: startup.showStartupSplash !== false,
    profileReady,
    themeReady: themeCatalog !== null,
  });

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

  if (splashVisible) {
    return (
      <>
        <div className="native-titlebar" data-tauri-drag-region />
        <StartupSplash />
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
      <div className="shell">
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
                {snapshot.title}
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
              onClick={() => setSearchPaneOpen(true)}
            >
              <MagnifyingGlass size={14} />
              Search
              <kbd>{settings.keybindings.search}</kbd>
            </Button>
            <MenuRoot>
              <MenuTrigger
                render={
                  <IconButton label="Create resource">
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
              resources={snapshot.resources}
              selectedPaths={selectedPaths}
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
              activeFolderPath={browserActiveFolderPath}
              onActiveFolderChange={setBrowserActiveFolderPath}
            />
          </nav>
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
              <button type="button" onClick={() => setActivityArea("home")}>
                {snapshot.title}
              </button>
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
                <span className={`save-state save-state-${saveState.status}`}>
                  {externalConflict ? "Conflict" : saveIndicatorText(saveState) || "Saved"}
                </span>
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
                  {tab.path === selected?.path && isUnsaved(saveState) && <i />}
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
            <section className="content-pane">
              {activityArea === "home" && (
                <HomeDashboard
                  title={snapshot.title}
                  resourceCount={snapshot.resources.length}
                  onNewPage={handleNewPage}
                  onQuickCapture={handleQuickNote}
                  onFiles={() => setActivityArea("files")}
                  onSearch={() => setSearchPaneOpen(true)}
                  onInspect={() => setInspectorOpen(true)}
                />
              )}

              {activityArea === "settings" && (
                <SettingsPage
                  settings={settings}
                  startup={startup}
                  workspace={snapshot}
                  themeCatalog={themeCatalog}
                  onChange={setSettings}
                  onStartupChange={setStartup}
                  onWorkspaceChange={(next) => void updateWorkspaceSettings(next)}
                  onClearRecents={clearRecents}
                  onReset={resetSettings}
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
                />
              )}

              {activityArea !== "home" && activityArea !== "settings" && (
                session?.kind === "canvas" ? (
                  <div className="canvas-pane">
                    <ResourceSurface
                      session={session}
                      capabilities={snapshot.capabilities}
                      context={{
                        assetRoot,
                        workspaceRoot: inBrowser ? null : snapshot.root,
                        resources: snapshot.resources,
                        settings,
                        pageEditorRef,
                        wikiTargets,
                        conflict: externalConflict,
                        reloadToken,
                        callbacks: {
                          onSaveStateChange: setSaveState,
                          onRevisionChange: handleRevisionChange,
                          onNotebookContentChange: handleNotebookContentChange,
                          onOpenWiki: (target) => {
                            void handleOpenWiki(target);
                            if (settings.editor.linkClickBehavior === "inspect") setInspectorOpen(true);
                          },
                          onCreateTable: handleNewTable,
                          onSearchWiki: !inBrowser
                            ? (query) => searchResourceLinks(snapshot.root, query, 20)
                            : undefined,
                          onImportAsset: inBrowser ? undefined : handleImportEditorAsset,
                          onKeepIncoming: () => void handleKeepIncoming(),
                          onKeepLocal: () => void handleKeepLocal(),
                          onKeepBoth: () => void handleKeepBoth(),
                          onOpenFile: handleOpenFile,
                          onOpenExternally: inBrowser ? undefined : (resource) => void handleOpenExternally(resource),
                          onPromoteWorkspaceCsv: inBrowser ? undefined : (resource) => void handlePromoteWorkspaceCsv(resource),
                          onPageWidthChange: (pageWidth) => setSettings((current) => ({
                            ...current,
                            editor: { ...current.editor, pageWidth },
                          })),
                        },
                      }}
                    />
                  </div>
                ) : (
                  <div className="main-scroll">
                    {!selected && (
                      <div className="placeholder">
                        <p className="placeholder-copy">Select a resource from Files.</p>
                        <p className="placeholder-sub">⌘N opens Quick Note · {QUICK_NOTE_SHORTCUT} works globally</p>
                      </div>
                    )}
                    {session && (
                      <ResourceSurface
                        session={session}
                        capabilities={snapshot.capabilities}
                        context={{
                          assetRoot,
                          workspaceRoot: inBrowser ? null : snapshot.root,
                          resources: snapshot.resources,
                          settings,
                          pageEditorRef,
                          wikiTargets,
                          conflict: externalConflict,
                          reloadToken,
                          callbacks: {
                            onSaveStateChange: setSaveState,
                            onRevisionChange: handleRevisionChange,
                            onNotebookContentChange: handleNotebookContentChange,
                            onOpenWiki: (target) => {
                              void handleOpenWiki(target);
                              if (settings.editor.linkClickBehavior === "inspect") setInspectorOpen(true);
                            },
                            onCreateTable: handleNewTable,
                            onSearchWiki: !inBrowser
                              ? (query) => searchResourceLinks(snapshot.root, query, 20)
                              : undefined,
                            onImportAsset: inBrowser ? undefined : handleImportEditorAsset,
                            onKeepIncoming: () => void handleKeepIncoming(),
                            onKeepLocal: () => void handleKeepLocal(),
                            onKeepBoth: () => void handleKeepBoth(),
                            onOpenFile: handleOpenFile,
                            onOpenExternally: inBrowser ? undefined : (resource) => void handleOpenExternally(resource),
                            onPromoteWorkspaceCsv: inBrowser ? undefined : (resource) => void handlePromoteWorkspaceCsv(resource),
                            onPageWidthChange: (pageWidth) => setSettings((current) => ({
                              ...current,
                              editor: { ...current.editor, pageWidth },
                            })),
                          },
                        }}
                      />
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
                dataSnapshot={session?.kind === "data-app" ? session.snapshot : null}
                error={error}
                onClose={() => setInspectorOpen(false)}
                onOpenFile={handleOpenFile}
              />
            )}
          </div>

          {terminalOpen && (
            <TerminalPanel
              workspaceRoot={inBrowser ? null : snapshot.root}
              hasTerminalCapability={hasCapability("terminal")}
              onClose={() => setTerminalOpen(false)}
            />
          )}
        </main>

      {paletteOpen && (
        <CommandPalette items={paletteItems} onClose={() => setPaletteOpen(false)} />
      )}
      {searchPaneOpen && (
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
      {statusToast && <div className="status-toast">{statusToast}</div>}
    </div>
    </TooltipProvider>
  );
}
