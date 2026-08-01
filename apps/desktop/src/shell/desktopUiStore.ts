/**
 * Per-window desktop UI control store.
 *
 * Owns shell chrome and cross-surface control state only — not workspace
 * documents, Pixi scenes, Tiptap transactions, or agent transcripts.
 * Surfaces publish save status here so typing does not re-render the shell.
 */
import { createContext, createElement, useContext, useRef, type ReactNode } from "react";
import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";

import type { SaveState } from "../editor/saveState";
import { DIRTY_SAVE_STATE, IDLE_SAVE_STATE } from "../editor/saveState";

export type DesktopUiState = {
  saveState: SaveState;
  sidebarWidth: number;
  inspectorOpen: boolean;
  agentPanelOpen: boolean;
  paletteOpen: boolean;
  searchPaneOpen: boolean;
  setSaveState: (state: SaveState | ((prev: SaveState) => SaveState)) => void;
  setSidebarWidth: (width: number) => void;
  setInspectorOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setAgentPanelOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setPaletteOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setSearchPaneOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
};

export type DesktopUiStore = StoreApi<DesktopUiState>;

export function createDesktopUiStore(
  initial?: Partial<Pick<DesktopUiState, "sidebarWidth">>,
): DesktopUiStore {
  return createStore<DesktopUiState>((set, get) => ({
    saveState: IDLE_SAVE_STATE,
    sidebarWidth: initial?.sidebarWidth ?? 272,
    inspectorOpen: false,
    agentPanelOpen: false,
    paletteOpen: false,
    searchPaneOpen: false,
    setSaveState: (state) => {
      const next = typeof state === "function" ? state(get().saveState) : state;
      if (next === get().saveState) return;
      if (next.status === "idle" && get().saveState.status === "idle") return;
      if (next.status === "dirty" && get().saveState.status === "dirty") return;
      set({ saveState: next });
    },
    setSidebarWidth: (sidebarWidth) => set({ sidebarWidth }),
    setInspectorOpen: (open) =>
      set({
        inspectorOpen: typeof open === "function" ? open(get().inspectorOpen) : open,
      }),
    setAgentPanelOpen: (open) =>
      set({
        agentPanelOpen: typeof open === "function" ? open(get().agentPanelOpen) : open,
      }),
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
