import { Button } from "@lattice/ui";
import {
  CursorText,
  Database,
  Files,
  Gauge,
  Keyboard,
  Microphone,
  Palette,
  Pulse,
  PuzzlePiece,
  Rocket,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import type { ThemeCatalogPayload } from "../theme";
import type { WorkspaceStartupSettings } from "../lib/profile";
import type { PageWidth } from "../lib/pageWidth";
import { getVoiceStatus, listenVoiceEvents, prepareVoiceModel, type VoiceStatus } from "../lib/voice";
import type { WorkspaceSnapshot } from "../types";
import { HistoryRetentionSettings } from "./HistoryRetentionSettings";
import type { AppSettings } from "./model";
import { TOGGLEABLE_WORKSPACE_CAPABILITIES } from "./workspaceCapabilities";

type SettingsSection =
  | "appearance"
  | "editor"
  | "files"
  | "workspaces"
  | "keybindings"
  | "data"
  | "capabilities"
  | "voice"
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
}

const SECTIONS = [
  { id: "appearance" as const, label: "Appearance", icon: Palette },
  { id: "editor" as const, label: "Editor behavior", icon: CursorText },
  { id: "files" as const, label: "Files, links & autosave", icon: Files },
  { id: "workspaces" as const, label: "Workspaces & startup", icon: Rocket },
  { id: "keybindings" as const, label: "Keybindings", icon: Keyboard },
  { id: "data" as const, label: "Data defaults", icon: Database },
  { id: "capabilities" as const, label: "Enabled capabilities", icon: PuzzlePiece },
  { id: "voice" as const, label: "Voice dictation", icon: Microphone },
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
}: SettingsPageProps) {
  const [section, setSection] = useState<SettingsSection>("appearance");
  const [quickNoteDraft, setQuickNoteDraft] = useState(workspace.defaults.quickNoteDirectory);
  const [defaultWorkspaceDraft, setDefaultWorkspaceDraft] = useState(
    startup.defaultWorkspace ?? "",
  );

  useEffect(() => {
    setQuickNoteDraft(workspace.defaults.quickNoteDirectory);
  }, [workspace.defaults.quickNoteDirectory]);

  useEffect(() => {
    setDefaultWorkspaceDraft(startup.defaultWorkspace ?? "");
  }, [startup.defaultWorkspace]);

  function update<K extends Exclude<keyof AppSettings, "format" | "version">>(
    group: K,
    patch: Partial<AppSettings[K]>,
  ) {
    onChange({ ...settings, [group]: { ...settings[group], ...patch } });
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
          </>
        )}

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
              even when an optional renderer is hidden.
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

        {section === "voice" && <VoiceDictationSettings />}

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

function VoiceDictationSettings() {
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

  const engineLabel = (() => {
    if (!status) return "Checking…";
    if (!status.available) return "Unavailable";
    if (status.preparing || busy) return "Preparing…";
    if (status.prepared) return "Ready";
    return "Available (not prepared)";
  })();

  return (
    <>
      <h1>Voice dictation</h1>
      <p>
        Local, on-device speech-to-text via FluidAudio / Parakeet Unified. Hold the microphone
        button in the page header to dictate. Provisional text never enters document storage.
      </p>
      {inBrowser ? (
        <div className="diagnostics-card">
          <strong>Unavailable in browser demo</strong>
          <span>Voice requires the native macOS desktop build with the FluidAudio bridge.</span>
        </div>
      ) : (
        <>
          <SettingRow
            title="Engine status"
            description="Availability of the local recognition runtime on this Mac."
          >
            <span>{engineLabel}</span>
          </SettingRow>
          <SettingRow
            title="Prepare model"
            description="Download and warm Parakeet Unified (~first run may take several minutes)."
          >
            <Button
              size="sm"
              disabled={busy || status?.available === false || status?.prepared === true}
              onClick={() => {
                setBusy(true);
                setError(null);
                void prepareVoiceModel()
                  .then((next) => setStatus(next))
                  .catch((err: unknown) =>
                    setError(err instanceof Error ? err.message : String(err)),
                  )
                  .finally(() => setBusy(false));
              }}
            >
              {busy ? "Preparing…" : status?.prepared ? "Prepared" : "Prepare now"}
            </Button>
          </SettingRow>
          {status?.message && (
            <div className="diagnostics-card">
              <span>{status.message}</span>
            </div>
          )}
          {error && (
            <div className="diagnostics-card" role="alert">
              <strong>Voice error</strong>
              <span>{error}</span>
            </div>
          )}
        </>
      )}
    </>
  );
}
