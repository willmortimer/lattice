import { Button } from "@lattice/ui";
import {
  CursorText,
  Cloud,
  Database,
  Files,
  Gauge,
  Keyboard,
  Lock,
  MagnifyingGlass,
  Microphone,
  Package,
  Palette,
  Plugs,
  Pulse,
  PuzzlePiece,
  Robot,
  Rocket,
  SquaresFour,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import type { ThemeCatalogPayload } from "../theme";
import type { AiMode, EmbeddingMode, WorkspaceStartupSettings } from "../lib/profile";
import type { PageWidth } from "../lib/pageWidth";
import { enableAppLock, getAppLockStatus, type AppLockStatus } from "../lib/appLock";
import {
  clearOpenaiApiKey,
  hasOpenaiApiKey,
  setOpenaiApiKey,
} from "../lib/openaiKey";
import {
  getVoiceStatus,
  listenVoiceEvents,
  prepareVoiceModel,
  VOICE_MODEL_CONFIRM,
  voicePackProviderLabel,
  voiceStatusLabel,
  type VoiceStatus,
} from "../lib/voice";
import {
  cloudSignIn,
  cloudSignInApple,
  cloudSignOut,
  getCloudSessionStatus,
  type CloudSessionStatus,
} from "../lib/cloud";
import {
  disableSemanticSearch,
  enableSemanticSearch,
  getSemanticStatus,
  isVectorsBehindStatus,
  listenSemanticEvents,
  SEMANTIC_MODEL_CONFIRM,
  semanticProviderLabel,
  semanticStatusLabel,
  VECTORS_BEHIND_EXPLANATION,
  type SemanticStatus,
} from "../lib/semantic";
import type { WorkspaceSnapshot } from "../types";
import {
  getBackgroundScheduleStatus,
  setBackgroundSchedulesEnabled,
  type BackgroundScheduleStatus,
} from "../lib/backgroundSchedules";
import { EmbeddingPackSettings } from "./EmbeddingPackSettings";
import { FeaturesSettings } from "./FeaturesSettings";
import { HistoryRetentionSettings } from "./HistoryRetentionSettings";
import type { AppSettings } from "./model";
import { PacksSettings } from "./PacksSettings";
import { PluginsSettings } from "./PluginsSettings";
import { TOGGLEABLE_WORKSPACE_CAPABILITIES } from "./workspaceCapabilities";

type SettingsSection =
  | "appearance"
  | "cloud"
  | "editor"
  | "files"
  | "workspaces"
  | "keybindings"
  | "data"
  | "capabilities"
  | "features"
  | "packs"
  | "plugins"
  | "ai"
  | "search"
  | "voice"
  | "privacy"
  | "performance"
  | "diagnostics";

interface SettingsPageProps {
  settings: AppSettings;
  startup: WorkspaceStartupSettings;
  workspace: WorkspaceSnapshot;
  themeCatalog: ThemeCatalogPayload | null;
  onChange: (next: AppSettings) => void;
  onStartupChange: (next: WorkspaceStartupSettings) => void;
  onWorkspaceChange: (next: {
    capabilities: string[];
    quickNoteDirectory: string;
  }) => void;
  onClearRecents: () => void;
  onReset: () => void;
  onThemeChange: (themeId: string) => void;
  onFollowSystem: () => void;
  onFontPackChange: (fontPack: string) => void;
  onRefreshProfile?: () => void;
}

const SECTIONS = [
  { id: "appearance" as const, label: "Appearance", icon: Palette },
  { id: "cloud" as const, label: "Cloud account", icon: Cloud },
  { id: "editor" as const, label: "Editor behavior", icon: CursorText },
  { id: "files" as const, label: "Files, links & autosave", icon: Files },
  { id: "workspaces" as const, label: "Workspaces & startup", icon: Rocket },
  { id: "keybindings" as const, label: "Keybindings", icon: Keyboard },
  { id: "data" as const, label: "Data defaults", icon: Database },
  { id: "capabilities" as const, label: "Enabled capabilities", icon: PuzzlePiece },
  { id: "features" as const, label: "Features", icon: SquaresFour },
  { id: "packs" as const, label: "Packs", icon: Package },
  { id: "plugins" as const, label: "Plugins", icon: Plugs },
  { id: "ai" as const, label: "AI", icon: Robot },
  { id: "search" as const, label: "Search", icon: MagnifyingGlass },
  { id: "voice" as const, label: "Voice dictation", icon: Microphone },
  { id: "privacy" as const, label: "Privacy", icon: Lock },
  { id: "performance" as const, label: "Performance & lifecycle", icon: Gauge },
  { id: "diagnostics" as const, label: "Advanced diagnostics", icon: Pulse },
];

function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div className="setting-row">
      <div>
        <strong>{title}</strong>
        <span>{description}</span>
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`settings-toggle ${checked ? "settings-toggle-on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function SettingsPage({
  settings,
  startup,
  workspace,
  themeCatalog,
  onChange,
  onStartupChange,
  onWorkspaceChange,
  onClearRecents,
  onReset,
  onThemeChange,
  onFollowSystem,
  onFontPackChange,
  onRefreshProfile,
}: SettingsPageProps) {
  const [section, setSection] = useState<SettingsSection>("appearance");
  const [quickNoteDraft, setQuickNoteDraft] = useState(workspace.defaults.quickNoteDirectory);
  const [defaultWorkspaceDraft, setDefaultWorkspaceDraft] = useState(
    startup.defaultWorkspace ?? "",
  );
  const [backgroundSchedules, setBackgroundSchedules] = useState<BackgroundScheduleStatus | null>(
    null,
  );
  const [backgroundSchedulesError, setBackgroundSchedulesError] = useState<string | null>(null);

  useEffect(() => {
    setQuickNoteDraft(workspace.defaults.quickNoteDirectory);
  }, [workspace.defaults.quickNoteDirectory]);

  useEffect(() => {
    setDefaultWorkspaceDraft(startup.defaultWorkspace ?? "");
  }, [startup.defaultWorkspace]);

  useEffect(() => {
    if (inBrowser || !workspace.root) {
      setBackgroundSchedules(null);
      setBackgroundSchedulesError(null);
      return;
    }
    let cancelled = false;
    void getBackgroundScheduleStatus(workspace.root)
      .then((status) => {
        if (!cancelled) {
          setBackgroundSchedules(status);
          setBackgroundSchedulesError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setBackgroundSchedules(null);
          setBackgroundSchedulesError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [workspace.root]);

  function update<K extends Exclude<keyof AppSettings, "format" | "version">>(
    group: K,
    patch: Partial<AppSettings[K]>,
  ) {
    onChange({ ...settings, [group]: { ...settings[group], ...patch } });
  }

  async function onToggleBackgroundSchedules(enabled: boolean) {
    if (!workspace.root || inBrowser) {
      return;
    }
    try {
      const status = await setBackgroundSchedulesEnabled(workspace.root, enabled);
      setBackgroundSchedules(status);
      setBackgroundSchedulesError(null);
    } catch (err: unknown) {
      setBackgroundSchedulesError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <div className="settings-workbench">
      <aside className="settings-nav">
        <p>Settings</p>
        {SECTIONS.map(({ id, label, icon: Icon }) => (
          <button
            type="button"
            key={id}
            className={section === id ? "settings-nav-active" : ""}
            onClick={() => setSection(id)}
          >
            <Icon size={15} />
            {label}
          </button>
        ))}
        <div className="settings-nav-spacer" />
        <Button variant="ghost" size="sm" onClick={onReset}>
          Reset defaults
        </Button>
      </aside>

      <section className="settings-detail">
        <p className="home-eyebrow">{SECTIONS.find((item) => item.id === section)?.label}</p>

        {section === "appearance" && (
          <>
            <h1>Appearance and themes</h1>
            <p className="settings-copy">
              Rust resolves semantic theme roles; shell components consume tokens rather than
              branching on a theme name.
            </p>
            <div className="theme-settings-grid">
              {(themeCatalog?.themes ?? []).map((theme) => (
                <button
                  type="button"
                  key={theme.id}
                  className={themeCatalog?.resolved.id === theme.id ? "theme-setting-active" : ""}
                  onClick={() => onThemeChange(theme.id)}
                >
                  <span className={`theme-swatch theme-swatch-${theme.id}`} />
                  <strong>{theme.name}</strong>
                  <small>{theme.appearance}</small>
                </button>
              ))}
            </div>
            <Button variant="secondary" onClick={onFollowSystem}>
              Follow system appearance
            </Button>
            <SettingRow
              title="Font pack"
              description="Typography stacks for display, UI, and mono. Follow theme uses each theme’s default pack (Cupertino → Apple)."
            >
              <select
                aria-label="Font pack"
                value={themeCatalog?.resolved.settings.fontPack ?? "theme"}
                onChange={(event) => onFontPackChange(event.target.value)}
              >
                <option value="theme">Follow theme</option>
                {(themeCatalog?.fontPacks ?? []).map((pack) => (
                  <option key={pack.id} value={pack.id}>
                    {pack.name}
                    {themeCatalog?.resolved.fontPack === pack.id &&
                    themeCatalog.resolved.settings.fontPack !== "theme"
                      ? " (active)"
                      : ""}
                  </option>
                ))}
              </select>
            </SettingRow>
            {themeCatalog?.resolved.settings.fontPack === "theme" && (
              <p className="settings-copy">
                Active pack: <strong>{themeCatalog.resolved.fontPack}</strong> (from theme)
              </p>
            )}
          </>
        )}

        {section === "cloud" && <CloudAccountSettings />}

        {section === "editor" && (
          <>
            <h1>Editor behavior</h1>
            <SettingRow title="Slash commands" description="Show block commands after typing / on an empty line.">
              <Toggle
                label="Slash commands"
                checked={settings.editor.slashCommands}
                onChange={(slashCommands) => update("editor", { slashCommands })}
              />
            </SettingRow>
            <SettingRow title="Spellcheck" description="Use the platform WebView spellchecker while editing pages.">
              <Toggle
                label="Spellcheck"
                checked={settings.editor.spellcheck}
                onChange={(spellcheck) => update("editor", { spellcheck })}
              />
            </SettingRow>
            <SettingRow title="Frontmatter" description="Expose raw YAML metadata above the page body.">
              <Toggle
                label="Show frontmatter"
                checked={settings.editor.showFrontmatter}
                onChange={(showFrontmatter) => update("editor", { showFrontmatter })}
              />
            </SettingRow>
            <SettingRow title="Link click" description="Choose whether a link navigates immediately or opens Inspect first.">
              <select
                value={settings.editor.linkClickBehavior}
                onChange={(event) =>
                  update("editor", {
                    linkClickBehavior: event.currentTarget.value as "navigate" | "inspect",
                  })
                }
              >
                <option value="navigate">Navigate</option>
                <option value="inspect">Inspect first</option>
              </select>
            </SettingRow>
            <SettingRow
              title="Page width"
              description="How wide the page column is. Standard keeps a readable measure; wide and full use more of the window."
            >
              <select
                value={settings.editor.pageWidth}
                onChange={(event) =>
                  update("editor", {
                    pageWidth: event.currentTarget.value as PageWidth,
                  })
                }
              >
                <option value="standard">Standard</option>
                <option value="wide">Wide</option>
                <option value="full">Full</option>
              </select>
            </SettingRow>
          </>
        )}

        {section === "files" && (
          <>
            <h1>Files, links and autosave</h1>
            <SettingRow title="Autosave delay" description="Debounce page writes while typing.">
              <select
                value={settings.editor.autosaveDelayMs}
                onChange={(event) =>
                  update("editor", { autosaveDelayMs: Number(event.currentTarget.value) })
                }
              >
                <option value="300">300 ms</option>
                <option value="800">800 ms</option>
                <option value="1500">1.5 seconds</option>
                <option value="3000">3 seconds</option>
              </select>
            </SettingRow>
            <SettingRow title="Quick Note folder" description="Workspace-relative directory for new captures.">
              <input
                value={quickNoteDraft}
                onChange={(event) => setQuickNoteDraft(event.currentTarget.value)}
                onBlur={() =>
                  onWorkspaceChange({
                    capabilities: workspace.capabilities,
                    quickNoteDirectory: quickNoteDraft,
                  })
                }
              />
            </SettingRow>
            <SettingRow title="Unsaved close guard" description="Require confirmation before closing a resource with local edits.">
              <Toggle
                label="Confirm unsaved close"
                checked={settings.files.confirmCloseWithUnsavedChanges}
                onChange={(confirmCloseWithUnsavedChanges) =>
                  update("files", { confirmCloseWithUnsavedChanges })
                }
              />
            </SettingRow>
          </>
        )}

        {section === "workspaces" && (
          <>
            <h1>Workspaces and startup</h1>
            <SettingRow title="Default workspace" description="Used when no valid session can be resumed.">
              <input
                value={defaultWorkspaceDraft}
                placeholder="No configured default"
                onChange={(event) => setDefaultWorkspaceDraft(event.currentTarget.value)}
                onBlur={() =>
                  onStartupChange({
                    ...startup,
                    defaultWorkspace: defaultWorkspaceDraft || null,
                  })
                }
              />
            </SettingRow>
            <SettingRow title="Reopen last workspace" description="Try recent workspaces before the configured default.">
              <Toggle
                label="Reopen last workspace"
                checked={startup.reopenLastWorkspace}
                onChange={(reopenLastWorkspace) =>
                  onStartupChange({ ...startup, reopenLastWorkspace })
                }
              />
            </SettingRow>
            <SettingRow title="Restore session" description="Restore tabs, active resource, activity area, and inspector state.">
              <Toggle
                label="Restore session"
                checked={startup.restoreSession}
                onChange={(restoreSession) =>
                  onStartupChange({ ...startup, restoreSession })
                }
              />
            </SettingRow>
            <SettingRow
              title="Startup splash"
              description="Hold the branded loading screen for about a second so theme colors can settle before the workspace appears."
            >
              <Toggle
                label="Show startup splash"
                checked={startup.showStartupSplash}
                onChange={(showStartupSplash) =>
                  onStartupChange({ ...startup, showStartupSplash })
                }
              />
            </SettingRow>
            <SettingRow title="Recent workspaces" description="Remove operational history without touching workspace files.">
              <Button variant="secondary" onClick={onClearRecents}>
                Clear recents
              </Button>
            </SettingRow>
          </>
        )}

        {section === "keybindings" && (
          <>
            <h1>Keybindings</h1>
            {(Object.entries(settings.keybindings) as Array<
              [keyof AppSettings["keybindings"], string]
            >).map(([key, value]) => (
              <SettingRow
                key={key}
                title={key.replace(/([A-Z])/g, " $1")}
                description="Use Mod for Command on macOS and Control elsewhere."
              >
                <input
                  className="keybinding-input"
                  value={value}
                  onChange={(event) => update("keybindings", { [key]: event.currentTarget.value })}
                />
              </SettingRow>
            ))}
          </>
        )}

        {section === "data" && (
          <>
            <h1>Data defaults</h1>
            <SettingRow title="Row density" description="Default canvas-grid row height.">
              <select
                value={settings.data.rowHeight}
                onChange={(event) =>
                  update("data", {
                    rowHeight: event.currentTarget.value as AppSettings["data"]["rowHeight"],
                  })
                }
              >
                <option value="compact">Compact</option>
                <option value="comfortable">Comfortable</option>
                <option value="spacious">Spacious</option>
              </select>
            </SettingRow>
            <SettingRow title="Query page size" description="Maximum rows requested in the current bounded table snapshot.">
              <select
                value={settings.data.pageSize}
                onChange={(event) =>
                  update("data", {
                    pageSize: Number(event.currentTarget.value) as AppSettings["data"]["pageSize"],
                  })
                }
              >
                <option value="100">100 rows</option>
                <option value="250">250 rows</option>
                <option value="500">500 rows</option>
              </select>
            </SettingRow>
            <SettingRow title="Row numbers" description="Keep a stable visual index beside grid records.">
              <Toggle
                label="Show row numbers"
                checked={settings.data.showRowNumbers}
                onChange={(showRowNumbers) => update("data", { showRowNumbers })}
              />
            </SettingRow>
            <SettingRow title="Zebra rows" description="Add a subtle alternating row tint.">
              <Toggle
                label="Zebra rows"
                checked={settings.data.zebraRows}
                onChange={(zebraRows) => update("data", { zebraRows })}
              />
            </SettingRow>
          </>
        )}

        {section === "capabilities" && (
          <>
            <h1>Enabled capabilities</h1>
            <p className="settings-copy">
              These switches control bundled shell surfaces. Canonical formats remain readable
              even when an optional renderer is hidden. Semantic search and voice packs live under
              Features and Packs.
            </p>
            {TOGGLEABLE_WORKSPACE_CAPABILITIES.map(({ key, title, description }) => (
              <SettingRow key={key} title={title} description={description}>
                <Toggle
                  label={key}
                  checked={workspace.capabilities.includes(key)}
                  onChange={(checked) =>
                    onWorkspaceChange({
                      capabilities: checked
                        ? [...workspace.capabilities, key]
                        : workspace.capabilities.filter((capability) => capability !== key),
                      quickNoteDirectory: workspace.defaults.quickNoteDirectory,
                    })
                  }
                />
              </SettingRow>
            ))}
            <div className="diagnostics-card">
              <strong>Always available</strong>
              <span>Pages, files, folders, search, Quick Capture, and external open.</span>
            </div>
          </>
        )}

        {section === "features" && (
          <FeaturesSettings
            workspaceRoot={workspace.root || null}
            semanticEnabled={settings.search.semanticEnabled}
            onSemanticEnabledChange={(semanticEnabled) => update("search", { semanticEnabled })}
            onOpenPacks={() => setSection("packs")}
            onOpenCapabilities={() => setSection("capabilities")}
          />
        )}

        {section === "packs" && (
          <PacksSettings
            workspaceRoot={workspace.root || null}
            onSemanticEnabledChange={(semanticEnabled) => update("search", { semanticEnabled })}
          />
        )}

        {section === "plugins" && <PluginsSettings />}

        {section === "ai" && (
          <AiSettingsPanel
            ai={settings.ai}
            workspaceRoot={workspace.root || null}
            semanticEnabled={settings.search.semanticEnabled}
            onSemanticEnabledChange={(semanticEnabled) => update("search", { semanticEnabled })}
            onChange={(patch) => update("ai", patch)}
            onOpenCloud={() => setSection("cloud")}
            onOpenVoice={() => setSection("voice")}
            onOpenPacks={() => setSection("packs")}
            onOpenFeatures={() => setSection("features")}
          />
        )}

        {section === "search" && (
          <SemanticSearchSettings
            workspaceRoot={workspace.root || null}
            semanticEnabled={settings.search.semanticEnabled}
            onSemanticEnabledChange={(semanticEnabled) => update("search", { semanticEnabled })}
            onOpenAi={() => setSection("ai")}
            onOpenFeatures={() => setSection("features")}
            onOpenPacks={() => setSection("packs")}
          />
        )}

        {section === "voice" && (
          <VoiceDictationSettings onOpenPacks={() => setSection("packs")} />
        )}

        {section === "privacy" && (
          <PrivacySettingsPanel
            settings={settings}
            onChange={onChange}
            onRefreshProfile={onRefreshProfile}
          />
        )}

        {section === "performance" && (
          <>
            <h1>Performance and lifecycle</h1>
            <SettingRow title="Maximum open tabs" description="Bound session state and renderer retention.">
              <input
                type="number"
                min="3"
                max="40"
                value={settings.performance.maxOpenTabs}
                onChange={(event) =>
                  update("performance", {
                    maxOpenTabs: Math.max(3, Math.min(40, Number(event.currentTarget.value))),
                  })
                }
              />
            </SettingRow>
            <SettingRow title="Suspend inactive resources" description="Unmount specialized renderers when their tab is inactive.">
              <Toggle
                label="Suspend inactive resources"
                checked={settings.performance.suspendInactiveResources}
                onChange={(suspendInactiveResources) =>
                  update("performance", { suspendInactiveResources })
                }
              />
            </SettingRow>
            <SettingRow title="Motion" description="Override animation and transition behavior.">
              <select
                value={settings.performance.reducedMotion}
                onChange={(event) =>
                  update("performance", {
                    reducedMotion: event.currentTarget.value as AppSettings["performance"]["reducedMotion"],
                  })
                }
              >
                <option value="system">Follow system</option>
                <option value="always">Reduce motion</option>
                <option value="never">Allow motion</option>
              </select>
            </SettingRow>
            <SettingRow title="Renderer cache" description="Retention policy for expensive lazy renderer modules and snapshots.">
              <select
                value={settings.performance.rendererCache}
                onChange={(event) =>
                  update("performance", {
                    rendererCache: event.currentTarget.value as AppSettings["performance"]["rendererCache"],
                  })
                }
              >
                <option value="conservative">Conservative</option>
                <option value="balanced">Balanced</option>
                <option value="aggressive">Aggressive</option>
              </select>
            </SettingRow>
            <h2 className="settings-subsection">Background services</h2>
            <SettingRow
              title="Keep app in menu bar"
              description="When enabled, closing the main window hides Lattice instead of quitting. Restore from the tray menu or Quit there to exit. This is not a login item."
            >
              <Toggle
                label="Keep app in menu bar"
                checked={settings.services.keepAppInMenuBar}
                onChange={(keepAppInMenuBar) => update("services", { keepAppInMenuBar })}
              />
            </SettingRow>
            <SettingRow
              title="Keep services running"
              description="Leave latticed running after the last desktop client disconnects so voice and search stay warm."
            >
              <Toggle
                label="Keep services running"
                checked={settings.services.keepServicesRunning}
                onChange={(keepServicesRunning) => update("services", { keepServicesRunning })}
              />
            </SettingRow>
            <SettingRow
              title="Allow background schedules"
              description="Opt this workspace into interval schedule runs while the desktop is closed. Requires latticed; holds a scheduler lease so idle shutdown does not stop the daemon. Cron is still deferred."
            >
              <Toggle
                label="Allow background schedules"
                checked={Boolean(backgroundSchedules?.enabled)}
                onChange={(enabled) => {
                  void onToggleBackgroundSchedules(enabled);
                }}
              />
            </SettingRow>
            {backgroundSchedulesError ? (
              <p className="settings-copy" role="status">
                Background schedules unavailable: {backgroundSchedulesError}
              </p>
            ) : null}
            {backgroundSchedules?.lastError ? (
              <p className="settings-copy" role="status">
                Last schedule error: {backgroundSchedules.lastError}
              </p>
            ) : null}
            <h2 className="settings-subsection">Revision history retention</h2>
            <HistoryRetentionSettings
              workspaceRoot={workspace.root || null}
              nativeAvailable={!inBrowser}
            />
          </>
        )}

        {section === "diagnostics" && (
          <>
            <h1>Advanced diagnostics</h1>
            <SettingRow title="Native context menus" description="Replace the WebView inspector menu with platform edit menus.">
              <Toggle
                label="Native context menus"
                checked={settings.diagnostics.nativeContextMenus}
                onChange={(nativeContextMenus) =>
                  update("diagnostics", { nativeContextMenus })
                }
              />
            </SettingRow>
            <SettingRow title="Command timings" description="Record frontend-to-command duration in the developer console.">
              <Toggle
                label="Command timings"
                checked={settings.diagnostics.commandTimings}
                onChange={(commandTimings) => update("diagnostics", { commandTimings })}
              />
            </SettingRow>
            <SettingRow title="Verbose errors" description="Show underlying command details in problems and diagnostics.">
              <Toggle
                label="Verbose errors"
                checked={settings.diagnostics.verboseErrors}
                onChange={(verboseErrors) => update("diagnostics", { verboseErrors })}
              />
            </SettingRow>
            <SettingRow title="Renderer statistics" description="Expose loaded-row and visible-cell diagnostics on data surfaces.">
              <Toggle
                label="Renderer statistics"
                checked={settings.diagnostics.showRendererStats}
                onChange={(showRendererStats) =>
                  update("diagnostics", { showRendererStats })
                }
              />
            </SettingRow>
            <div className="diagnostics-card">
              <strong>Desktop runtime</strong>
              <span>Tauri 2 · React 19 · lazy page/canvas/grid renderers</span>
              <span>Canonical mutations: Rust semantic command core</span>
            </div>
          </>
        )}
      </section>
    </div>
  );
}

const AI_MODE_OPTIONS: Array<{
  id: AiMode;
  label: string;
  description: string;
}> = [
  {
    id: "local",
    label: "Local",
    description: "Apple-native and on-device models. No cloud API key required.",
  },
  {
    id: "byoOpenai",
    label: "BYO OpenAI",
    description: "Use your own OpenAI API key from the OS keychain.",
  },
  {
    id: "account",
    label: "Account",
    description: "Lattice-mediated OpenAI via your cloud account (coming soon).",
  },
];

function AiSettingsPanel({
  ai,
  workspaceRoot,
  semanticEnabled,
  onSemanticEnabledChange,
  onChange,
  onOpenCloud,
  onOpenVoice,
  onOpenPacks,
  onOpenFeatures,
}: {
  ai: AppSettings["ai"];
  workspaceRoot: string | null;
  semanticEnabled: boolean;
  onSemanticEnabledChange: (semanticEnabled: boolean) => void;
  onChange: (patch: Partial<AppSettings["ai"]>) => void;
  onOpenCloud: () => void;
  onOpenVoice: () => void;
  onOpenPacks: () => void;
  onOpenFeatures: () => void;
}) {
  const [hasKey, setHasKey] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [preferredModelDraft, setPreferredModelDraft] = useState(ai.preferredModel ?? "");
  const [cloudStatus, setCloudStatus] = useState<CloudSessionStatus | null>(null);

  useEffect(() => {
    setPreferredModelDraft(ai.preferredModel ?? "");
  }, [ai.preferredModel]);

  useEffect(() => {
    if (inBrowser) {
      setHasKey(false);
      return;
    }
    let cancelled = false;
    void hasOpenaiApiKey()
      .then((present) => {
        if (!cancelled) setHasKey(present);
      })
      .catch((err: unknown) => {
        if (!cancelled) setKeyError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (inBrowser || ai.mode !== "account") return;
    let cancelled = false;
    void getCloudSessionStatus()
      .then((next) => {
        if (!cancelled) setCloudStatus(next);
      })
      .catch(() => {
        if (!cancelled) setCloudStatus(null);
      });
    return () => {
      cancelled = true;
    };
  }, [ai.mode]);

  async function handleSaveKey() {
    if (inBrowser) return;
    const trimmed = keyDraft.trim();
    if (!trimmed) return;
    setKeyBusy(true);
    setKeyError(null);
    try {
      await setOpenaiApiKey(trimmed);
      setHasKey(true);
      setKeyDraft("");
    } catch (err: unknown) {
      setKeyError(err instanceof Error ? err.message : String(err));
    } finally {
      setKeyBusy(false);
    }
  }

  async function handleClearKey() {
    if (inBrowser) return;
    setKeyBusy(true);
    setKeyError(null);
    try {
      await clearOpenaiApiKey();
      setHasKey(false);
      setKeyDraft("");
    } catch (err: unknown) {
      setKeyError(err instanceof Error ? err.message : String(err));
    } finally {
      setKeyBusy(false);
    }
  }

  function commitPreferredModel() {
    const trimmed = preferredModelDraft.trim();
    onChange({ preferredModel: trimmed.length > 0 ? trimmed : null });
  }

  return (
    <>
      <h1>AI</h1>
      <p className="settings-copy">
        Choose how the agent and related features reach a model. Embedding and voice downloads are
        managed under Packs; feature toggles under Features.
      </p>

      <div className="ai-mode-choices" role="radiogroup" aria-label="AI mode">
        {AI_MODE_OPTIONS.map((option) => {
          const active = ai.mode === option.id;
          return (
            <button
              type="button"
              key={option.id}
              role="radio"
              aria-checked={active}
              className={`ai-mode-choice${active ? " ai-mode-choice-active" : ""}`}
              onClick={() => onChange({ mode: option.id })}
            >
              <span className="ai-mode-choice-indicator" aria-hidden="true" />
              <span>
                <strong>{option.label}</strong>
                <span>{option.description}</span>
              </span>
            </button>
          );
        })}
      </div>

      {ai.mode === "local" ? (
        <div className="diagnostics-card" role="status">
          <strong>Local mode</strong>
          <span>
            Prefer on-device inference. Agent and embedding paths follow local providers when
            available; no OpenAI key is required.
          </span>
        </div>
      ) : null}

      {ai.mode === "byoOpenai" ? (
        <>
          <SettingRow
            title="OpenAI API key"
            description="Stored in the OS keychain. Lattice only checks whether a key is present — the secret is never shown."
          >
            <span>{hasKey ? "Key on file" : "No key stored"}</span>
          </SettingRow>
          {inBrowser ? (
            <div className="diagnostics-card">
              <strong>Unavailable in browser demo</strong>
              <span>Keychain storage requires the native desktop shell.</span>
            </div>
          ) : (
            <>
              <SettingRow
                title={hasKey ? "Replace key" : "Set key"}
                description="Paste an OpenAI API key, then save. Clearing removes it from the keychain."
              >
                <div className="ai-key-row">
                  <input
                    type="password"
                    autoComplete="off"
                    spellCheck={false}
                    placeholder="sk-…"
                    value={keyDraft}
                    disabled={keyBusy}
                    aria-label="OpenAI API key"
                    onChange={(event) => setKeyDraft(event.currentTarget.value)}
                  />
                  <Button
                    size="sm"
                    disabled={keyBusy || !keyDraft.trim()}
                    onClick={() => void handleSaveKey()}
                  >
                    {keyBusy ? "Saving…" : "Save key"}
                  </Button>
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={keyBusy || !hasKey}
                    onClick={() => void handleClearKey()}
                  >
                    Clear key
                  </Button>
                </div>
              </SettingRow>
              {keyError ? (
                <div className="diagnostics-card" role="alert">
                  <strong>OpenAI key error</strong>
                  <span>{keyError}</span>
                </div>
              ) : null}
            </>
          )}
        </>
      ) : null}

      {ai.mode === "account" ? (
        <div className="diagnostics-card" role="status">
          <strong>Account mode</strong>
          <span>
            Lattice-mediated OpenAI (project key via your cloud account) is coming soon. Sign in
            under Cloud account when you are ready; this mode is not Pioneer-first.
          </span>
          <div className="ai-account-actions">
            <Button size="sm" variant="secondary" onClick={onOpenCloud}>
              {cloudStatus?.signedIn ? "Open Cloud account" : "Sign in to Cloud"}
            </Button>
          </div>
        </div>
      ) : null}

      <h2 className="settings-subsection">Model preference</h2>
      <SettingRow
        title="Preferred model"
        description="Optional model id hint for the agent (for example gpt-4o-mini). Leave blank to use the runtime default."
      >
        <input
          value={preferredModelDraft}
          placeholder="Runtime default"
          aria-label="Preferred model"
          onChange={(event) => setPreferredModelDraft(event.currentTarget.value)}
          onBlur={() => commitPreferredModel()}
        />
      </SettingRow>

      <h2 className="settings-subsection">Embedding defaults</h2>
      <SettingRow
        title="Embedding mode"
        description="Follow AI uses the same provider family as the AI mode. Local and remote override that choice."
      >
        <select
          aria-label="Embedding mode"
          value={ai.embeddingMode}
          onChange={(event) =>
            onChange({ embeddingMode: event.currentTarget.value as EmbeddingMode })
          }
        >
          <option value="followAi">Follow AI mode</option>
          <option value="local">Local</option>
          <option value="remote">Remote</option>
        </select>
      </SettingRow>
      <SettingRow
        title="Passive embedding"
        description="Allow background embedding when the workspace is idle. Pack download and vector freshness are under Embeddings below."
      >
        <Toggle
          label="Passive embedding"
          checked={ai.passiveEmbeddingEnabled}
          onChange={(passiveEmbeddingEnabled) => onChange({ passiveEmbeddingEnabled })}
        />
      </SettingRow>

      <EmbeddingPackSettings
        workspaceRoot={workspaceRoot}
        semanticEnabled={semanticEnabled}
        onSemanticEnabledChange={onSemanticEnabledChange}
        embeddingMode={ai.embeddingMode}
        passiveEmbeddingEnabled={ai.passiveEmbeddingEnabled}
      />

      <h2 className="settings-subsection">Optional packs</h2>
      <p className="settings-copy">Managed under Packs and Features.</p>
      <SettingRow
        title="Embedding & voice packs"
        description="Download status, clear, and feature enablement live in the Packs and Features panels."
      >
        <div className="history-retention-actions">
          <Button size="sm" variant="secondary" onClick={onOpenPacks}>
            Open Packs
          </Button>
          <Button size="sm" variant="secondary" onClick={onOpenFeatures}>
            Open Features
          </Button>
          <Button size="sm" variant="secondary" onClick={onOpenVoice}>
            Voice details
          </Button>
        </div>
      </SettingRow>
    </>
  );
}

function SemanticSearchSettings({
  workspaceRoot,
  semanticEnabled,
  onSemanticEnabledChange,
  onOpenAi,
  onOpenFeatures,
  onOpenPacks,
}: {
  workspaceRoot: string | null;
  semanticEnabled: boolean;
  onSemanticEnabledChange: (semanticEnabled: boolean) => void;
  onOpenAi: () => void;
  onOpenFeatures: () => void;
  onOpenPacks: () => void;
}) {
  const [status, setStatus] = useState<SemanticStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser || !workspaceRoot) return;
    let cancelled = false;
    void getSemanticStatus(workspaceRoot)
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot]);

  // Single owner for enable/disable — do not also invoke from the toggle handler
  // (that raced two downloads on the same .partial and produced jumpy % / missing artifact).
  useEffect(() => {
    if (inBrowser || !workspaceRoot) return;
    let cancelled = false;
    setBusy(true);
    setError(null);
    const op = semanticEnabled
      ? enableSemanticSearch(workspaceRoot)
      : disableSemanticSearch(workspaceRoot);
    void op
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : String(err));
        if (semanticEnabled) {
          // Roll preference back so we do not loop a failed enable.
          onSemanticEnabledChange(false);
        }
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
    // Intentionally omit onSemanticEnabledChange — parent passes an inline lambda.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- preference + root only
  }, [workspaceRoot, semanticEnabled]);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenSemanticEvents((event) => {
      if (event.type === "status") {
        setStatus((prev) => {
          const nextPercent = event.progressPercent ?? null;
          // Keep progress monotonic while downloading so polling / events cannot flicker backward.
          const progressPercent =
            event.state === "downloading" &&
            prev?.state === "downloading" &&
            prev.progressPercent != null &&
            nextPercent != null
              ? Math.max(prev.progressPercent, nextPercent)
              : nextPercent;
          return {
            state: event.state,
            pendingChunks: event.pendingChunks,
            message: event.message,
            progressPercent,
            providerId: event.providerId ?? prev?.providerId ?? null,
            modelId: event.modelId ?? prev?.modelId ?? null,
            dimensions: event.dimensions ?? prev?.dimensions ?? null,
          };
        });
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  // Poll while downloading/preparing/indexing so progress stays fresh.
  useEffect(() => {
    if (inBrowser || !workspaceRoot || !semanticEnabled) return;
    if (
      !status ||
      (status.state !== "downloading" &&
        status.state !== "preparing" &&
        status.state !== "indexing")
    ) {
      return;
    }
    const id = window.setInterval(() => {
      void getSemanticStatus(workspaceRoot)
        .then((next) => {
          setStatus((prev) => {
            if (
              next.state === "downloading" &&
              prev?.state === "downloading" &&
              prev.progressPercent != null &&
              next.progressPercent != null
            ) {
              return {
                ...next,
                progressPercent: Math.max(prev.progressPercent, next.progressPercent),
              };
            }
            return next;
          });
        })
        .catch(() => {
          /* keep last status */
        });
    }, 750);
    return () => window.clearInterval(id);
  }, [workspaceRoot, semanticEnabled, status?.state]);

  function handleToggle(next: boolean) {
    if (next) {
      const accepted = window.confirm(SEMANTIC_MODEL_CONFIRM);
      if (!accepted) return;
    }
    // Preference only — the effect above starts/stops the worker exactly once.
    onSemanticEnabledChange(next);
  }

  const statusText = status
    ? semanticStatusLabel(
        status.state,
        status.pendingChunks,
        status.progressPercent,
        status.message,
      )
    : semanticEnabled
      ? "Preparing…"
      : "Not prepared";
  const providerText = status ? semanticProviderLabel(status) : null;
  const vectorsBehind = status != null && isVectorsBehindStatus(status);

  return (
    <>
      <h1>Search</h1>
      <p className="settings-copy">
        Keyword search is always available. Semantic search uses a local embedding model to find
        related passages by meaning, not just exact words. Managed under Features; pack download
        under Packs.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Semantic search requires the native desktop build with latticed indexing services.</span>
        </div>
      ) : (
        <>
          <SettingRow
            title="Semantic search"
            description="Include vector similarity alongside keyword matches when searching the workspace."
          >
            <Toggle
              label="Semantic search"
              checked={semanticEnabled}
              onChange={(checked) => void handleToggle(checked)}
            />
          </SettingRow>
          <SettingRow
            title="Index status"
            description="Whether the local embedding model and workspace index are ready."
          >
            <span>
              {busy ? (
                "Updating…"
              ) : (
                <>
                  {statusText}
                  {providerText ? (
                    <>
                      <br />
                      <span className="settings-copy">Provider: {providerText}</span>
                    </>
                  ) : null}
                </>
              )}
            </span>
          </SettingRow>
          <SettingRow
            title="Embedding pack"
            description="Download, clear, and feature toggle. Vector freshness also appears under AI → Embeddings."
          >
            <div className="history-retention-actions">
              <Button size="sm" variant="secondary" onClick={onOpenFeatures}>
                Open Features
              </Button>
              <Button size="sm" variant="secondary" onClick={onOpenPacks}>
                Open Packs
              </Button>
              <Button size="sm" variant="secondary" onClick={onOpenAi}>
                AI → Embeddings
              </Button>
            </div>
          </SettingRow>
          {vectorsBehind ? (
            <div className="diagnostics-card" role="status">
              <strong>Vectors behind workspace</strong>
              <span>{VECTORS_BEHIND_EXPLANATION}</span>
            </div>
          ) : null}
          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>Semantic search error</strong>
              <span>{error}</span>
            </div>
          ) : null}
          {status?.message && status.state === "failed" ? (
            <div className="diagnostics-card" role="status">
              <strong>Details</strong>
              <span>{status.message}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}

function PrivacySettingsPanel({
  settings,
  onChange,
  onRefreshProfile,
}: {
  settings: AppSettings;
  onChange: (next: AppSettings) => void;
  onRefreshProfile?: () => void;
}) {
  const [lockStatus, setLockStatus] = useState<AppLockStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const privacy = settings.privacy ?? { appLockEnabled: false, idleLockMinutes: 5 };

  useEffect(() => {
    if (inBrowser) return;
    let cancelled = false;
    void getAppLockStatus()
      .then((status) => {
        if (!cancelled) setLockStatus(status);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const platformSupported = lockStatus?.platformSupported ?? false;
  const presenceAvailable = lockStatus?.presenceAvailable ?? false;

  return (
    <>
      <h1>Privacy</h1>
      <p className="settings-copy">
        App lock hides the Lattice session and refuses privileged desktop commands until you
        authenticate. It does not encrypt workspace files on disk — content remains inspectable
        outside Lattice.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>App lock requires the native macOS desktop build with Touch ID or device password.</span>
        </div>
      ) : !platformSupported ? (
        <div className="diagnostics-card">
          <strong>macOS only</strong>
          <span>App lock uses LocalAuthentication and is not available on this platform.</span>
        </div>
      ) : (
        <>
          <SettingRow
            title="App lock"
            description="Require Touch ID or your device password when Lattice launches, after idle, and from Lattice → Lock."
          >
            <Toggle
              label="App lock"
              checked={privacy.appLockEnabled}
              onChange={(enabled) => {
                if (!enabled) {
                  onChange({
                    ...settings,
                    privacy: { ...privacy, appLockEnabled: false },
                  });
                  return;
                }
                setBusy(true);
                setError(null);
                void enableAppLock(privacy.idleLockMinutes)
                  .then((status) => {
                    setLockStatus(status);
                    // Rust already persisted privacy settings; reload revision
                    // instead of writing again through the debounced settings save.
                    onRefreshProfile?.();
                  })
                  .catch((err: unknown) => {
                    setError(err instanceof Error ? err.message : String(err));
                  })
                  .finally(() => setBusy(false));
              }}
            />
          </SettingRow>
          <SettingRow
            title="Idle lock (minutes)"
            description="Lock after the main window is unfocused for this many minutes. Use 0 to disable idle auto-lock (launch, manual Lock, and sleep still apply)."
          >
            <input
              type="number"
              min={0}
              max={120}
              value={privacy.idleLockMinutes}
              disabled={busy}
              onChange={(event) => {
                const idleLockMinutes = Math.max(
                  0,
                  Math.min(120, Number(event.currentTarget.value) || 0),
                );
                onChange({
                  ...settings,
                  privacy: { ...privacy, idleLockMinutes },
                });
              }}
            />
          </SettingRow>
          {!presenceAvailable ? (
            <p className="settings-copy" role="status">
              Device authentication is not available right now. Enroll Touch ID or set a
              password in System Settings.
            </p>
          ) : null}
          {busy ? <p className="settings-copy">Waiting for authentication…</p> : null}
          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>App lock error</strong>
              <span>{error}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}

function CloudAccountSettings() {
  const [status, setStatus] = useState<CloudSessionStatus | null>(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser) return;
    let cancelled = false;
    void getCloudSessionStatus()
      .then((next) => {
        if (!cancelled) {
          setStatus(next);
          if (next.user?.email) setEmail(next.user.email);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSignIn() {
    if (inBrowser) return;
    setBusy(true);
    setError(null);
    try {
      const next = await cloudSignIn(email.trim(), password);
      setStatus(next);
      setPassword("");
      if (next.error) setError(next.error);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleAppleSignIn() {
    if (inBrowser) return;
    setBusy(true);
    setError(null);
    try {
      const next = await cloudSignInApple();
      setStatus(next);
      if (next.user?.email) setEmail(next.user.email);
      if (next.error) setError(next.error);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleSignOut() {
    if (inBrowser) return;
    setBusy(true);
    setError(null);
    try {
      const next = await cloudSignOut();
      setStatus(next);
      setPassword("");
      if (next.error) setError(next.error);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  const statusText = status
    ? status.signedIn
      ? status.user?.email ?? status.user?.display_name ?? "Signed in"
      : "Not signed in"
    : "Checking…";

  return (
    <>
      <h1>Cloud account</h1>
      <p className="settings-copy">
        Optional lattice-server sign-in for cloud sync features. Credentials stay in the OS
        keychain; the bearer token never reaches browser storage.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Cloud sign-in requires the native desktop shell.</span>
        </div>
      ) : (
        <>
          <SettingRow
            title="Server"
            description="Resolved lattice-server origin for this desktop build."
          >
            <span>{status?.cloudUrl ?? "https://cloud.lattice-notes.com"}</span>
          </SettingRow>
          <SettingRow title="Status" description="Current cloud session for this device.">
            <span>{busy ? "Updating…" : statusText}</span>
          </SettingRow>
          {!status?.signedIn ? (
            <>
              <SettingRow title="Apple" description="Same Sign in with Apple account as the web app.">
                <Button size="sm" disabled={busy} onClick={() => void handleAppleSignIn()}>
                  {busy ? "Signing in…" : "Sign in with Apple"}
                </Button>
              </SettingRow>
              <SettingRow title="Email" description="Account email for password login.">
                <input
                  type="email"
                  autoComplete="username"
                  value={email}
                  disabled={busy}
                  onChange={(event) => setEmail(event.currentTarget.value)}
                />
              </SettingRow>
              <SettingRow title="Password" description="Password for your lattice-server account.">
                <input
                  type="password"
                  autoComplete="current-password"
                  value={password}
                  disabled={busy}
                  onChange={(event) => setPassword(event.currentTarget.value)}
                />
              </SettingRow>
              <Button
                size="sm"
                disabled={busy || !email.trim() || !password}
                onClick={() => void handleSignIn()}
              >
                {busy ? "Signing in…" : "Sign in with password"}
              </Button>
            </>
          ) : (
            <Button size="sm" variant="secondary" disabled={busy} onClick={() => void handleSignOut()}>
              {busy ? "Signing out…" : "Sign out"}
            </Button>
          )}
          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>Cloud sign-in error</strong>
              <span>{error}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}

function VoiceDictationSettings({ onOpenPacks }: { onOpenPacks: () => void }) {
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inBrowser) return;
    let cancelled = false;
    void getVoiceStatus()
      .then((next) => {
        if (!cancelled) {
          setStatus(next);
          if (next.preparing) setBusy(true);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenVoiceEvents((event) => {
      if (event.type === "status") {
        if (event.state === "preparing") {
          setBusy(true);
          setError(null);
          setStatus((prev) =>
            prev
              ? { ...prev, preparing: true, message: event.message }
              : {
                  available: true,
                  prepared: false,
                  preparing: true,
                  listening: false,
                  nativeCapture: false,
                  platform: "macos",
                  message: event.message,
                },
          );
        }
        if (event.state === "ready") {
          setBusy(false);
          setError(null);
          setStatus((prev) =>
            prev
              ? { ...prev, prepared: true, preparing: false, message: event.message }
              : {
                  available: true,
                  prepared: true,
                  preparing: false,
                  listening: false,
                  nativeCapture: false,
                  platform: "macos",
                  message: event.message,
                },
          );
        }
        if (event.state === "idle") {
          setBusy(false);
        }
      }
      if (event.type === "failed") {
        setBusy(false);
        setError(event.message);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  function handleDownloadPack() {
    if (status?.prepared || busy || status?.available === false) return;
    const accepted = window.confirm(VOICE_MODEL_CONFIRM);
    if (!accepted) return;
    setBusy(true);
    setError(null);
    void prepareVoiceModel()
      .then((next) => setStatus(next))
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  }

  const statusText = voiceStatusLabel(status, { busy, error });
  const providerText = voicePackProviderLabel(status);
  const packUnavailable = status != null && !status.available;
  const downloadLabel = busy
    ? "Downloading…"
    : status?.prepared
      ? "Downloaded"
      : "Download pack";

  return (
    <>
      <h1>Voice dictation</h1>
      <p className="settings-copy">
        Optional on-device speech-to-text. Download a local recognition pack once, then hold the
        microphone in the page header to dictate. Audio stays on this Mac; provisional text never
        enters document storage. Managed under Packs.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Voice requires the native macOS desktop build with the FluidAudio bridge.</span>
        </div>
      ) : packUnavailable ? (
        <div className="diagnostics-card" role="status">
          <strong>Voice pack unavailable</strong>
          <span>
            {status.message?.trim() ||
              "Local recognition requires the native macOS desktop build with the FluidAudio bridge."}
          </span>
        </div>
      ) : (
        <>
          <SettingRow
            title="Voice pack"
            description="Download Parakeet Unified (~608 MB) for local dictation. First prepare may take several minutes."
          >
            <div className="history-retention-actions">
              <Button
                size="sm"
                disabled={busy || status?.prepared === true || status == null}
                onClick={() => void handleDownloadPack()}
              >
                {downloadLabel}
              </Button>
              <Button size="sm" variant="secondary" onClick={onOpenPacks}>
                Open Packs
              </Button>
            </div>
          </SettingRow>
          <SettingRow
            title="Pack status"
            description="Whether the FluidAudio recognition pack is ready on this Mac."
          >
            <span>
              {statusText}
              {providerText ? (
                <>
                  <br />
                  <span className="settings-copy">Provider: {providerText}</span>
                </>
              ) : null}
            </span>
          </SettingRow>
          {error ? (
            <div className="diagnostics-card" role="alert">
              <strong>Voice pack error</strong>
              <span>{error}</span>
            </div>
          ) : null}
          {status?.message && (busy || status.preparing || error) ? (
            <div className="diagnostics-card" role="status">
              <strong>Details</strong>
              <span>{status.message}</span>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
