import { Button } from "@lattice/ui";
import {
  Activity,
  Database,
  FileCog,
  Gauge,
  Keyboard,
  Palette,
  Puzzle,
  Rocket,
  TextCursorInput,
} from "lucide-react";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import type { ThemeCatalogPayload } from "../theme";
import type { WorkspaceStartupSettings } from "../lib/profile";
import type { WorkspaceSnapshot } from "../types";
import { HistoryRetentionSettings } from "./HistoryRetentionSettings";
import type { AppSettings } from "./model";

type SettingsSection =
  | "appearance"
  | "editor"
  | "files"
  | "workspaces"
  | "keybindings"
  | "data"
  | "capabilities"
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
  { id: "editor" as const, label: "Editor behavior", icon: TextCursorInput },
  { id: "files" as const, label: "Files, links & autosave", icon: FileCog },
  { id: "workspaces" as const, label: "Workspaces & startup", icon: Rocket },
  { id: "keybindings" as const, label: "Keybindings", icon: Keyboard },
  { id: "data" as const, label: "Data defaults", icon: Database },
  { id: "capabilities" as const, label: "Enabled capabilities", icon: Puzzle },
  { id: "performance" as const, label: "Performance & lifecycle", icon: Gauge },
  { id: "diagnostics" as const, label: "Advanced diagnostics", icon: Activity },
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
            {(["canvas", "sqlite"] as const).map((key) => (
              <SettingRow
                key={key}
                title={key === "sqlite" ? "Data apps" : "Canvas"}
                description="Workspace-owned and materialized through the semantic manifest command."
              >
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
