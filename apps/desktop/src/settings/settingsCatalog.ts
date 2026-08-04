import type { IconProps } from "@phosphor-icons/react";
import {
  Cloud,
  CursorText,
  Database,
  Files,
  Gauge,
  HardDrives,
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

export type SettingsScope = "APP" | "WORKSPACE" | "ACCOUNT" | "DEVICE";

export type SettingsSection =
  | "appearance"
  | "cloud"
  | "remote"
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

export interface SettingsNavItem {
  id: SettingsSection;
  label: string;
  icon: React.ComponentType<IconProps>;
}

export interface SettingsNavGroup {
  id: string;
  label: string;
  items: SettingsNavItem[];
}

export interface SettingsSearchItem {
  id: string;
  section: SettingsSection;
  title: string;
  description: string;
  keywords: string[];
}

const appearance: SettingsNavItem = { id: "appearance", label: "Appearance", icon: Palette };
const editor: SettingsNavItem = { id: "editor", label: "Editor behavior", icon: CursorText };
const files: SettingsNavItem = {
  id: "files",
  label: "Files, links & autosave",
  icon: Files,
};
const keybindings: SettingsNavItem = { id: "keybindings", label: "Keybindings", icon: Keyboard };
const workspaces: SettingsNavItem = {
  id: "workspaces",
  label: "Workspaces & startup",
  icon: Rocket,
};
const data: SettingsNavItem = { id: "data", label: "Data defaults", icon: Database };
const capabilities: SettingsNavItem = {
  id: "capabilities",
  label: "Enabled capabilities",
  icon: PuzzlePiece,
};
const search: SettingsNavItem = { id: "search", label: "Search", icon: MagnifyingGlass };
const ai: SettingsNavItem = { id: "ai", label: "AI", icon: Robot };
const voice: SettingsNavItem = { id: "voice", label: "Voice dictation", icon: Microphone };
const features: SettingsNavItem = { id: "features", label: "Features", icon: SquaresFour };
const packs: SettingsNavItem = { id: "packs", label: "Packs", icon: Package };
const plugins: SettingsNavItem = { id: "plugins", label: "Plugins", icon: Plugs };
const cloud: SettingsNavItem = { id: "cloud", label: "Cloud account", icon: Cloud };
const remote: SettingsNavItem = { id: "remote", label: "Remote access", icon: HardDrives };
const privacy: SettingsNavItem = { id: "privacy", label: "Privacy", icon: Lock };
const performance: SettingsNavItem = {
  id: "performance",
  label: "Performance & lifecycle",
  icon: Gauge,
};
const diagnostics: SettingsNavItem = {
  id: "diagnostics",
  label: "Advanced diagnostics",
  icon: Pulse,
};

export const SETTINGS_NAV_GROUPS: SettingsNavGroup[] = [
  {
    id: "general",
    label: "General",
    items: [appearance, editor, files, keybindings],
  },
  {
    id: "workspace",
    label: "Workspace",
    items: [workspaces, data, capabilities],
  },
  {
    id: "intelligence",
    label: "Intelligence",
    items: [search, ai, voice],
  },
  {
    id: "extensions",
    label: "Extensions",
    items: [features, packs, plugins],
  },
  {
    id: "account",
    label: "Account & connectivity",
    items: [cloud, remote],
  },
  {
    id: "system",
    label: "System",
    items: [privacy, performance, diagnostics],
  },
];

export const SETTINGS_SECTIONS: SettingsNavItem[] = SETTINGS_NAV_GROUPS.flatMap(
  (group) => group.items,
);

export function sectionLabel(section: SettingsSection): string {
  return SETTINGS_SECTIONS.find((item) => item.id === section)?.label ?? section;
}

export const SECTION_DEFAULT_SCOPE: Record<SettingsSection, SettingsScope> = {
  appearance: "APP",
  editor: "APP",
  files: "APP",
  keybindings: "APP",
  workspaces: "APP",
  data: "WORKSPACE",
  capabilities: "WORKSPACE",
  search: "WORKSPACE",
  ai: "APP",
  voice: "DEVICE",
  features: "WORKSPACE",
  packs: "DEVICE",
  plugins: "APP",
  cloud: "ACCOUNT",
  remote: "WORKSPACE",
  privacy: "APP",
  performance: "APP",
  diagnostics: "APP",
};

const SETTING_SCOPE_OVERRIDES: Record<string, SettingsScope> = {
  "files.quick-note": "WORKSPACE",
  "performance.schedules": "WORKSPACE",
  "performance.history": "WORKSPACE",
  "ai.openai-key": "DEVICE",
  "privacy.app-lock": "DEVICE",
  "privacy.idle-lock": "DEVICE",
  "privacy.ai-audit": "ACCOUNT",
  "privacy.telemetry": "ACCOUNT",
  "remote.access": "WORKSPACE",
};

export function sectionScope(section: SettingsSection): SettingsScope {
  return SECTION_DEFAULT_SCOPE[section];
}

export function settingScopeForId(settingId: string | undefined): SettingsScope | null {
  if (!settingId) return null;
  if (SETTING_SCOPE_OVERRIDES[settingId]) return SETTING_SCOPE_OVERRIDES[settingId];
  const section = settingId.split(".")[0] as SettingsSection;
  return SECTION_DEFAULT_SCOPE[section] ?? "APP";
}

export interface SettingsDeepLinkTarget {
  section: SettingsSection;
  settingId: string | null;
}

export const SETTINGS_SEARCH_INDEX: SettingsSearchItem[] = [
  {
    id: "appearance.theme",
    section: "appearance",
    title: "Theme",
    description: "Color appearance and semantic theme roles.",
    keywords: ["dark", "light", "cupertino", "theme", "colors"],
  },
  {
    id: "appearance.font-pack",
    section: "appearance",
    title: "Font pack",
    description: "Typography stacks for display, UI, and mono.",
    keywords: ["typography", "fonts", "display", "mono"],
  },
  {
    id: "editor.slash-commands",
    section: "editor",
    title: "Slash commands",
    description: "Show block commands after typing / on an empty line.",
    keywords: ["slash", "commands", "blocks"],
  },
  {
    id: "editor.spellcheck",
    section: "editor",
    title: "Spellcheck",
    description: "Use the platform WebView spellchecker while editing pages.",
    keywords: ["spelling", "grammar"],
  },
  {
    id: "editor.frontmatter",
    section: "editor",
    title: "Frontmatter",
    description: "Expose raw YAML metadata above the page body.",
    keywords: ["yaml", "metadata"],
  },
  {
    id: "editor.link-click",
    section: "editor",
    title: "Link click",
    description: "Choose whether a link navigates immediately or opens Inspect first.",
    keywords: ["links", "navigation", "inspect"],
  },
  {
    id: "editor.page-width",
    section: "editor",
    title: "Page width",
    description: "How wide the page column is.",
    keywords: ["layout", "column", "wide", "full"],
  },
  {
    id: "files.autosave",
    section: "files",
    title: "Autosave delay",
    description: "Debounce page writes while typing.",
    keywords: ["save", "debounce", "delay"],
  },
  {
    id: "files.quick-note",
    section: "files",
    title: "Quick Note folder",
    description: "Workspace-relative directory for new captures.",
    keywords: ["capture", "quick note", "folder"],
  },
  {
    id: "files.unsaved-close",
    section: "files",
    title: "Unsaved close guard",
    description: "Require confirmation before closing a resource with local edits.",
    keywords: ["confirm", "close", "unsaved"],
  },
  {
    id: "workspaces.default",
    section: "workspaces",
    title: "Default workspace",
    description: "Used when no valid session can be resumed.",
    keywords: ["startup", "open"],
  },
  {
    id: "workspaces.reopen",
    section: "workspaces",
    title: "Reopen last workspace",
    description: "Try recent workspaces before the configured default.",
    keywords: ["recent", "session"],
  },
  {
    id: "workspaces.restore-session",
    section: "workspaces",
    title: "Restore session",
    description: "Restore tabs, active resource, activity area, and inspector state.",
    keywords: ["tabs", "state"],
  },
  {
    id: "workspaces.splash",
    section: "workspaces",
    title: "Startup splash",
    description: "Hold the branded loading screen while theme colors settle.",
    keywords: ["loading", "boot"],
  },
  {
    id: "workspaces.clear-recents",
    section: "workspaces",
    title: "Recent workspaces",
    description: "Remove operational history without touching workspace files.",
    keywords: ["history", "recents"],
  },
  {
    id: "keybindings.shortcuts",
    section: "keybindings",
    title: "Keybindings",
    description: "Keyboard shortcuts for common actions.",
    keywords: ["keyboard", "shortcuts", "hotkeys", "mod"],
  },
  {
    id: "data.row-density",
    section: "data",
    title: "Row density",
    description: "Default canvas-grid row height.",
    keywords: ["grid", "compact", "comfortable"],
  },
  {
    id: "data.page-size",
    section: "data",
    title: "Query page size",
    description: "Maximum rows requested in the current bounded table snapshot.",
    keywords: ["pagination", "rows", "table"],
  },
  {
    id: "data.row-numbers",
    section: "data",
    title: "Row numbers",
    description: "Keep a stable visual index beside grid records.",
    keywords: ["index", "grid"],
  },
  {
    id: "data.zebra-rows",
    section: "data",
    title: "Zebra rows",
    description: "Add a subtle alternating row tint.",
    keywords: ["striped", "grid"],
  },
  {
    id: "capabilities.canvas",
    section: "capabilities",
    title: "Canvas",
    description: "Workspace-owned canvas renderer.",
    keywords: ["surface", "manifest"],
  },
  {
    id: "capabilities.sqlite",
    section: "capabilities",
    title: "Data apps",
    description: "Workspace-owned SQLite data apps.",
    keywords: ["sqlite", "database", "grid"],
  },
  {
    id: "capabilities.terminal",
    section: "capabilities",
    title: "Terminal",
    description: "Embedded shell dock in the activity rail.",
    keywords: ["shell", "command line"],
  },
  {
    id: "search.semantic",
    section: "search",
    title: "Semantic search",
    description: "Include vector similarity alongside keyword matches.",
    keywords: ["embeddings", "vectors", "hybrid", "fts"],
  },
  {
    id: "features.semantic",
    section: "features",
    title: "Semantic search",
    description: "Feature toggle for vector similarity search.",
    keywords: ["embeddings", "vectors"],
  },
  {
    id: "features.voice",
    section: "features",
    title: "Voice dictation",
    description: "Hold-to-talk speech-to-text once the voice pack is ready.",
    keywords: ["speech", "dictation", "microphone"],
  },
  {
    id: "features.memory",
    section: "features",
    title: "Agent memory",
    description: "Remember and recall for the embedded agent.",
    keywords: ["recall", "embeddings"],
  },
  {
    id: "features.labs-cloud-blob",
    section: "features",
    title: "Labs cloud blob",
    description: "Upload a workspace path to cloud or reopen cloud-authoritative bytes.",
    keywords: ["labs", "cloud", "blob", "materialize", "authority"],
  },
  {
    id: "features.labs-encrypted-backup",
    section: "features",
    title: "Labs encrypted workspace backup",
    description: "Encrypt a workspace snapshot with the DEK and PUT opaque bytes to cloud backup storage.",
    keywords: ["labs", "cloud", "backup", "encrypt", "dek", "ciphertext"],
  },
  {
    id: "features.labs-collaborative-page",
    section: "features",
    title: "Labs collaborative page editor",
    description: "Opt-in Yjs collaborative editing for pages with registry ResourceIds.",
    keywords: ["labs", "collab", "yjs", "tiptap", "collaborative"],
  },
  {
    id: "packs.catalog",
    section: "packs",
    title: "Downloadable packs",
    description: "Embedding and voice model packs.",
    keywords: ["download", "qwen", "parakeet", "models"],
  },
  {
    id: "plugins.catalog",
    section: "plugins",
    title: "Plugins",
    description: "Third-party shell extensions.",
    keywords: ["extensions", "third party"],
  },
  {
    id: "ai.mode",
    section: "ai",
    title: "AI mode",
    description: "How the workspace agent reaches a model.",
    keywords: ["local", "byo", "openai", "cloud", "paid"],
  },
  {
    id: "ai.openai-key",
    section: "ai",
    title: "OpenAI API key",
    description: "Stored in the OS keychain for BYO mode.",
    keywords: ["api", "keychain", "byo"],
  },
  {
    id: "ai.preferred-model",
    section: "ai",
    title: "Preferred model",
    description: "Allowlisted chat model for the agent.",
    keywords: ["gpt", "chat", "llm"],
  },
  {
    id: "ai.embedding-mode",
    section: "ai",
    title: "Embedding mode",
    description: "Follow AI, local pack, or remote OpenAI embeddings.",
    keywords: ["vectors", "indexing"],
  },
  {
    id: "ai.passive-embedding",
    section: "ai",
    title: "Passive embedding",
    description: "Allow background embedding when the workspace is idle.",
    keywords: ["background", "indexing"],
  },
  {
    id: "voice.pack",
    section: "voice",
    title: "Voice pack",
    description: "Download Parakeet for local dictation.",
    keywords: ["speech", "parakeet", "fluidaudio"],
  },
  {
    id: "cloud.sign-in",
    section: "cloud",
    title: "Cloud account",
    description: "Sign in to lattice-server for sync and Lattice paid AI.",
    keywords: ["apple", "password", "sync", "account"],
  },
  {
    id: "remote.access",
    section: "remote",
    title: "Remote access",
    description: "Advertise workspaces to Lattice Cloud for remote MCP tools.",
    keywords: ["mcp", "relay", "daemon"],
  },
  {
    id: "privacy.app-lock",
    section: "privacy",
    title: "App lock",
    description: "Require Touch ID, Windows Hello, or device PIN/password when Lattice launches.",
    keywords: ["touch id", "windows hello", "pin", "authentication", "security"],
  },
  {
    id: "privacy.idle-lock",
    section: "privacy",
    title: "Idle lock",
    description: "Lock after the main window is unfocused.",
    keywords: ["timeout", "authentication"],
  },
  {
    id: "privacy.ai-audit",
    section: "privacy",
    title: "AI request audit",
    description: "Record metadata-only request rows for Lattice paid AI.",
    keywords: ["logging", "audit"],
  },
  {
    id: "privacy.telemetry",
    section: "privacy",
    title: "Anonymous product telemetry",
    description: "Coarse product events only.",
    keywords: ["analytics", "tracking"],
  },
  {
    id: "performance.max-tabs",
    section: "performance",
    title: "Maximum open tabs",
    description: "Bound session state and renderer retention.",
    keywords: ["tabs", "memory"],
  },
  {
    id: "performance.suspend",
    section: "performance",
    title: "Suspend inactive resources",
    description: "Unmount specialized renderers when their tab is inactive.",
    keywords: ["lazy", "memory"],
  },
  {
    id: "performance.motion",
    section: "performance",
    title: "Motion",
    description: "Override animation and transition behavior.",
    keywords: ["animation", "reduced motion"],
  },
  {
    id: "performance.renderer-cache",
    section: "performance",
    title: "Renderer cache",
    description: "Retention policy for expensive lazy renderer modules.",
    keywords: ["cache", "memory"],
  },
  {
    id: "performance.menu-bar",
    section: "performance",
    title: "Keep app in menu bar",
    description: "Closing the main window hides Lattice instead of quitting.",
    keywords: ["tray", "background"],
  },
  {
    id: "performance.services",
    section: "performance",
    title: "Keep services running",
    description: "Leave latticed running after the last client disconnects.",
    keywords: ["daemon", "latticed"],
  },
  {
    id: "performance.schedules",
    section: "performance",
    title: "Background schedules",
    description: "Opt into interval schedule runs while the desktop is closed.",
    keywords: ["cron", "scheduler"],
  },
  {
    id: "performance.history",
    section: "performance",
    title: "Revision history retention",
    description: "How long page revisions are kept on disk.",
    keywords: ["versions", "snapshots"],
  },
  {
    id: "diagnostics.context-menus",
    section: "diagnostics",
    title: "Native context menus",
    description: "Replace the WebView inspector menu with platform edit menus.",
    keywords: ["right click", "menu"],
  },
  {
    id: "diagnostics.timings",
    section: "diagnostics",
    title: "Command timings",
    description: "Record frontend-to-command duration in the developer console.",
    keywords: ["performance", "debug"],
  },
  {
    id: "diagnostics.verbose-errors",
    section: "diagnostics",
    title: "Verbose errors",
    description: "Show underlying command details in problems and diagnostics.",
    keywords: ["debug", "errors"],
  },
  {
    id: "diagnostics.renderer-stats",
    section: "diagnostics",
    title: "Renderer statistics",
    description: "Expose loaded-row and visible-cell diagnostics on data surfaces.",
    keywords: ["debug", "grid"],
  },
];

export function filterSettingsSearch(query: string): SettingsSearchItem[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return [];
  const terms = normalized.split(/\s+/).filter(Boolean);
  return SETTINGS_SEARCH_INDEX.filter((item) => {
    const haystack = [item.title, item.description, ...item.keywords].join(" ").toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
}

const SETTINGS_DEEP_LINK_ALIASES: Record<string, string> = {
  "ai/provider": "ai.mode",
  "search/semantic": "search.semantic",
  "remote-access": "remote.access",
};

function catalogItemById(id: string) {
  return SETTINGS_SEARCH_INDEX.find((item) => item.id === id) ?? null;
}

function sectionBySlug(slug: string): SettingsSection | null {
  const normalized = slug.trim().toLowerCase();
  const match = SETTINGS_SECTIONS.find((item) => item.id === normalized);
  return match?.id ?? null;
}

/** Map a settings path (from lattice://settings/…) to a nav section and optional row id. */
export function resolveSettingsDeepLink(path: string): SettingsDeepLinkTarget | null {
  const normalized = path.trim().replace(/^\/+/, "").replace(/\/+$/, "").toLowerCase();
  if (!normalized) return null;

  const aliasId = SETTINGS_DEEP_LINK_ALIASES[normalized];
  if (aliasId) {
    const item = catalogItemById(aliasId);
    if (item) return { section: item.section, settingId: item.id };
  }

  const dotId = normalized.replace(/\//g, ".");
  const byDotId = catalogItemById(dotId);
  if (byDotId) return { section: byDotId.section, settingId: byDotId.id };

  const sectionOnly = sectionBySlug(normalized);
  if (sectionOnly) return { section: sectionOnly, settingId: null };

  const [first, ...rest] = normalized.split("/");
  const section = sectionBySlug(first);
  if (!section) return null;
  if (rest.length === 0) return { section, settingId: null };

  const nestedSlug = rest.join("/");
  const nestedAlias = SETTINGS_DEEP_LINK_ALIASES[`${first}/${nestedSlug}`];
  if (nestedAlias) {
    const item = catalogItemById(nestedAlias);
    if (item) return { section: item.section, settingId: item.id };
  }

  const nestedDotId = `${first}.${rest.join("-")}`;
  const nestedItem = catalogItemById(nestedDotId);
  if (nestedItem) return { section: nestedItem.section, settingId: nestedItem.id };

  const suffixMatch = SETTINGS_SEARCH_INDEX.find(
    (item) => item.section === section && item.id.endsWith(`.${rest.join("-")}`),
  );
  if (suffixMatch) return { section: suffixMatch.section, settingId: suffixMatch.id };

  return { section, settingId: null };
}
