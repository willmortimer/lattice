/**
 * Per-window desktop UI control store.
 *
 * Owns shell chrome and cross-surface control state only — not workspace
 * documents, Pixi scenes, Tiptap transactions, or agent transcripts.
 * Surfaces publish save status here so typing does not re-render the shell.
 * Save status is keyed by renderer session so split panes / multi-tab chrome
 * can subscribe independently.
 */
import { createContext, createElement, useContext, useRef, type ReactNode } from "react";
import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";

import {
  DEFAULT_DATA_TABLE_PANEL_SIZES,
  type DataTablePanelSizes,
} from "../data/dataTableLayout";
import type { SaveState } from "../editor/saveState";
import { DIRTY_SAVE_STATE, IDLE_SAVE_STATE } from "../editor/saveState";
import type { ResourceStat } from "../lib/resourceStat";
import {
  authorityBadgeForMode,
  type ResourceTreeAuthorityBadge,
  type ResourceTreeSyncBadge,
} from "../lib/resourceTreeBadges";
import type { WorkspaceCloudSyncSnapshot } from "../lib/cloudSync";

/**
 * Opaque id for a mounted resource renderer session.
 * Today this is the resource path (one active editor per path); split panes
 * may introduce pane-qualified ids later without changing the store shape.
 */
export type RendererSessionId = string;

export type AgentLayoutMode = "dock" | "workbench" | "focus" | "detached";

/** Non-focus in-window layout restored when leaving focus (Escape / Focus toggle). */
export type AgentFocusReturnMode = "dock" | "workbench";

export type AgentWorkbenchPanelSizes = {
  conversation: number;
  side: number;
};

function focusReturnModeFrom(mode: AgentLayoutMode): AgentFocusReturnMode {
  return mode === "workbench" ? "workbench" : "dock";
}

const DEFAULT_AGENT_WORKBENCH_PANEL_SIZES: AgentWorkbenchPanelSizes = {
  conversation: 58,
  side: 42,
};

/** Map a resource path to today's default renderer session id. */
export function rendererSessionIdForPath(path: string): RendererSessionId {
  return path;
}

export type DesktopUiState = {
  saveStatusBySessionId: Record<string, SaveState>;
  authorityBadgeByPath: Record<string, ResourceTreeAuthorityBadge>;
  syncBadgeByPath: Record<string, ResourceTreeSyncBadge>;
  workspaceCloudSync: WorkspaceCloudSyncSnapshot;
  sidebarWidth: number;
  inspectorOpen: boolean;
  helpOpen: boolean;
  agentPanelOpen: boolean;
  agentLayoutMode: AgentLayoutMode;
  agentFocusReturnMode: AgentFocusReturnMode;
  agentWorkbenchPanelSizes: AgentWorkbenchPanelSizes;
  dataTablePanelSizes: DataTablePanelSizes;
  paletteOpen: boolean;
  searchPaneOpen: boolean;
  setSaveStatus: (
    sessionId: RendererSessionId,
    state: SaveState | ((prev: SaveState) => SaveState),
  ) => void;
  clearSaveStatus: (sessionId: RendererSessionId) => void;
  clearAllSaveStatuses: () => void;
  remapSaveStatus: (fromSessionId: RendererSessionId, toSessionId: RendererSessionId) => void;
  recordAuthorityStat: (stat: ResourceStat) => void;
  clearAuthorityBadges: () => void;
  setSyncBadges: (badges: Record<string, ResourceTreeSyncBadge>) => void;
  clearSyncBadges: () => void;
  setWorkspaceCloudSync: (
    snapshot:
      | WorkspaceCloudSyncSnapshot
      | ((prev: WorkspaceCloudSyncSnapshot) => WorkspaceCloudSyncSnapshot),
  ) => void;
  setSidebarWidth: (width: number) => void;
  setInspectorOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setHelpOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setAgentPanelOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setAgentLayoutMode: (mode: AgentLayoutMode) => void;
  exitAgentFocus: () => void;
  setAgentWorkbenchPanelSizes: (sizes: AgentWorkbenchPanelSizes) => void;
  setDataTablePanelSizes: (sizes: DataTablePanelSizes) => void;
  setPaletteOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setSearchPaneOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
};

export type DesktopUiStore = StoreApi<DesktopUiState>;

export function saveStatusForSession(
  statuses: Record<string, SaveState>,
  sessionId: RendererSessionId | null | undefined,
): SaveState {
  if (!sessionId) return IDLE_SAVE_STATE;
  return statuses[sessionId] ?? IDLE_SAVE_STATE;
}

function resolveSaveStatus(
  prev: SaveState,
  state: SaveState | ((prev: SaveState) => SaveState),
): SaveState {
  return typeof state === "function" ? state(prev) : state;
}

function shouldSkipSaveStatusWrite(prev: SaveState, next: SaveState): boolean {
  if (next === prev) return true;
  if (next.status === "idle" && prev.status === "idle") return true;
  if (next.status === "dirty" && prev.status === "dirty") return true;
  return false;
}

export function createDesktopUiStore(
  initial?: Partial<Pick<DesktopUiState, "sidebarWidth">>,
): DesktopUiStore {
  return createStore<DesktopUiState>((set, get) => ({
    saveStatusBySessionId: {},
    authorityBadgeByPath: {},
    syncBadgeByPath: {},
    workspaceCloudSync: {
      phase: "idle",
      message: null,
      lastSyncedAt: null,
      conflictCount: 0,
      errorCount: 0,
      cloudWorkspaceId: null,
    },
    sidebarWidth: initial?.sidebarWidth ?? 272,
    inspectorOpen: false,
    helpOpen: false,
    agentPanelOpen: false,
    agentLayoutMode: "dock",
    agentFocusReturnMode: "dock",
    agentWorkbenchPanelSizes: DEFAULT_AGENT_WORKBENCH_PANEL_SIZES,
    dataTablePanelSizes: DEFAULT_DATA_TABLE_PANEL_SIZES,
    paletteOpen: false,
    searchPaneOpen: false,
    setSaveStatus: (sessionId, state) => {
      const prev = get().saveStatusBySessionId[sessionId] ?? IDLE_SAVE_STATE;
      const next = resolveSaveStatus(prev, state);
      if (shouldSkipSaveStatusWrite(prev, next)) return;
      set({
        saveStatusBySessionId: {
          ...get().saveStatusBySessionId,
          [sessionId]: next,
        },
      });
    },
    clearSaveStatus: (sessionId) => {
      const current = get().saveStatusBySessionId;
      if (!(sessionId in current)) return;
      const { [sessionId]: _removed, ...rest } = current;
      set({ saveStatusBySessionId: rest });
    },
    clearAllSaveStatuses: () => {
      if (Object.keys(get().saveStatusBySessionId).length === 0) return;
      set({ saveStatusBySessionId: {} });
    },
    recordAuthorityStat: (stat) => {
      const badge = authorityBadgeForMode(stat.authority);
      if (!badge) return;
      set({
        authorityBadgeByPath: {
          ...get().authorityBadgeByPath,
          [stat.path]: badge,
        },
      });
    },
    clearAuthorityBadges: () => {
      if (Object.keys(get().authorityBadgeByPath).length === 0) return;
      set({ authorityBadgeByPath: {} });
    },
    setSyncBadges: (badges) => {
      set({ syncBadgeByPath: badges });
    },
    clearSyncBadges: () => {
      if (Object.keys(get().syncBadgeByPath).length === 0) return;
      set({ syncBadgeByPath: {} });
    },
    setWorkspaceCloudSync: (snapshot) =>
      set({
        workspaceCloudSync:
          typeof snapshot === "function" ? snapshot(get().workspaceCloudSync) : snapshot,
      }),
    remapSaveStatus: (fromSessionId, toSessionId) => {
      if (fromSessionId === toSessionId) return;
      const current = get().saveStatusBySessionId;
      if (!(fromSessionId in current)) return;
      const { [fromSessionId]: status, ...rest } = current;
      set({
        saveStatusBySessionId: {
          ...rest,
          [toSessionId]: status ?? IDLE_SAVE_STATE,
        },
      });
    },
    setSidebarWidth: (sidebarWidth) => set({ sidebarWidth }),
    setInspectorOpen: (open) =>
      set({
        inspectorOpen: typeof open === "function" ? open(get().inspectorOpen) : open,
      }),
    setHelpOpen: (open) =>
      set({
        helpOpen: typeof open === "function" ? open(get().helpOpen) : open,
      }),
    setAgentPanelOpen: (open) =>
      set({
        agentPanelOpen: typeof open === "function" ? open(get().agentPanelOpen) : open,
      }),
    setAgentLayoutMode: (agentLayoutMode) =>
      set((state) => {
        if (agentLayoutMode === "focus" && state.agentLayoutMode !== "focus") {
          return {
            agentLayoutMode,
            agentFocusReturnMode: focusReturnModeFrom(state.agentLayoutMode),
          };
        }
        return { agentLayoutMode };
      }),
    exitAgentFocus: () =>
      set((state) => {
        if (state.agentLayoutMode !== "focus") return state;
        return { agentLayoutMode: state.agentFocusReturnMode };
      }),
    setAgentWorkbenchPanelSizes: (agentWorkbenchPanelSizes) => set({ agentWorkbenchPanelSizes }),
    setDataTablePanelSizes: (dataTablePanelSizes) => set({ dataTablePanelSizes }),
    setPaletteOpen: (open) =>
      set({
        paletteOpen: typeof open === "function" ? open(get().paletteOpen) : open,
      }),
    setSearchPaneOpen: (open) =>
      set({
        searchPaneOpen: typeof open === "function" ? open(get().searchPaneOpen) : open,
      }),
  }));
}

const DesktopUiStoreContext = createContext<DesktopUiStore | null>(null);

export function DesktopUiStoreProvider({
  store,
  children,
}: {
  store?: DesktopUiStore;
  children: ReactNode;
}) {
  const localRef = useRef<DesktopUiStore | null>(null);
  if (!localRef.current) {
    localRef.current = store ?? createDesktopUiStore();
  }
  return createElement(
    DesktopUiStoreContext.Provider,
    { value: localRef.current },
    children,
  );
}

export function useDesktopUiStoreApi(): DesktopUiStore {
  const store = useContext(DesktopUiStoreContext);
  if (!store) {
    throw new Error("useDesktopUiStoreApi requires DesktopUiStoreProvider");
  }
  return store;
}

export function useDesktopUiStore<T>(selector: (state: DesktopUiState) => T): T {
  const store = useDesktopUiStoreApi();
  return useStore(store, selector);
}

/** Stable constants re-exported for shell chrome. */
export { DIRTY_SAVE_STATE, IDLE_SAVE_STATE };
